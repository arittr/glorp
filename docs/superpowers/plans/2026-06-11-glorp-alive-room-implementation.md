# Glorp Alive Room Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Alive Room watch experience so earned props define a visibly distinct room, day/weather changes alter the room silhouette, the pet performs the state, and Preview Lab proves still and animated behavior.

**Spec:** `docs/superpowers/specs/2026-06-11-glorp-alive-room-design.md` — requirements and design decisions live there; do not restate them here.

**Architecture:** Add a pure `src/tui/room.rs` presentation layer for room profile derivation, keep prop placement as the single source for rendered cells and preview targets, and extend the existing Preview Lab/export path before adding room visuals. Extend the current pet animator into a scene animator with one `tachyonfx::EffectManager`; every enqueued effect gets an explicit target area, and room-level targets are split to avoid pet and speech cells.

**Tech Stack:** Rust, ratatui, tachyonfx 0.18, serde/serde_json, clap, insta, assert_cmd, Preview Lab.

**Branch guidance:** Work on the current branch unless Drew explicitly creates a new one. Commit after each task that reaches green verification. Do not push unless Drew asks.

---

## File Structure

| File | Status | Responsibility |
|---|---|---|
| `src/dev_preview/strips.rs` | create | deterministic multi-frame preview strip fixtures and helpers |
| `src/dev_preview/export.rs` | modify | `manifest.strips[]`, strip artifacts, review markdown, HTML rendering |
| `src/dev_preview/scenarios.rs` | modify | `PreviewSelection::Animation`, bundle orchestration, strip writing |
| `src/dev_preview/assets/preview.html` | modify | strip insertion point |
| `src/dev_preview/assets/preview.css` | modify | strip viewport and paused controls |
| `src/dev_preview/assets/preview.js` | modify | paused frame-by-frame strip playback |
| `src/cli.rs` / `src/commands/dev_preview.rs` | modify | hidden `--scenario animation` selector |
| `tests/dev_preview.rs` | modify | strip artifact, manifest, HTML, fixture, and visual-diff integration tests |
| `src/tui/room.rs` | create | `RoomLifeProfile`, biome weighting, emitters, pet performance, scene moments |
| `src/tui/mod.rs` | modify | expose `room` module |
| `src/tui/component/habitat_props.rs` | modify | shared prop placements, bounds, static prop target ids |
| `src/tui/component/geometry.rs` | modify | target roles for room and prop effects |
| `src/tui/component/pet_scene.rs` | modify | room target and effect target list |
| `src/tui/component/preview.rs` | modify | preview layout target metadata v2 |
| `src/tui/component/watch_screen.rs` | modify | render entry point that can pass scene targets to the animator |
| `src/tui/layout.rs` | modify | render entry point and target helpers |
| `src/tui/panels/pet.rs` | modify | Alive Room render passes and pet performance cues |
| `src/tui/app.rs` | modify | update/apply `SceneAnimator` with full watch effect targets |
| `src/pet/animator.rs` | modify | rename/extend pet animator into `SceneAnimator` |
| `src/dev_preview/watch.rs` | modify | Alive Room still fixtures and room profile manifest inputs |
| `src/dev_preview/habitat_props.rs` | modify | prop placement target proof fixtures |

## Task 1: Add Preview Strip Infrastructure First

**Spec sections:** Preview Lab Proof, Testing, Rollout Shape.

**Files:**
- Create: `src/dev_preview/strips.rs`
- Modify: `src/dev_preview/mod.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/assets/preview.html`
- Modify: `src/dev_preview/assets/preview.css`
- Modify: `src/dev_preview/assets/preview.js`
- Modify: `src/cli.rs`
- Modify: `src/commands/dev_preview.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing integration tests for animation-only output**

Append to `tests/dev_preview.rs` before the snapshot tests:

```rust
#[test]
fn dev_preview_animation_writes_scene_strip_manifest_and_frames() {
    let run = PreviewRun::new();

    run.run_success("animation");

    assert!(run.out.join("manifest.json").is_file());
    assert!(run.out.join("review.md").is_file());
    assert!(run.out.join("index.html").is_file());
    assert!(run.out.join("strips/scene-strip-smoke/frame-000.txt").is_file());
    assert!(run
        .out
        .join("strips/scene-strip-smoke/frame-000.cells.json")
        .is_file());
    assert!(!run.out.join("frames/watch-wide-normal.txt").exists());

    let manifest = run.manifest();
    assert_eq!(manifest["schema_version"], 2);
    assert!(
        manifest["scenarios"].as_array().unwrap().is_empty(),
        "animation-only bundles should not write static scenarios"
    );
    let strips = manifest["strips"].as_array().expect("strips should be an array");
    assert_eq!(strips.len(), 1);
    assert_eq!(strips[0]["id"], "scene-strip-smoke");
    assert_eq!(strips[0]["kind"], "scene-moment");
    assert_eq!(strips[0]["dimensions"]["width"], 40);
    assert_eq!(strips[0]["dimensions"]["height"], 8);
    assert_eq!(strips[0]["target_id"], "watch.room.effect");
    assert_eq!(strips[0]["frames"][0]["phase"], "start");
    assert_eq!(strips[0]["frames"][0]["elapsed_ms"], 0);
    assert_eq!(
        strips[0]["frames"][0]["files"]["text"],
        "strips/scene-strip-smoke/frame-000.txt"
    );
    assert_artifact_type(&manifest, "scene-strip-smoke-frame-000", "text");
    assert_artifact_type(&manifest, "scene-strip-smoke-frame-000-cells", "cells");
}

#[test]
fn dev_preview_all_includes_scene_strips() {
    let run = PreviewRun::new();

    run.run_success("all");

    let manifest = run.manifest();
    assert!(
        !manifest["strips"].as_array().unwrap().is_empty(),
        "all preview should include animation strips"
    );
    assert!(run.out.join("frames/watch-wide-normal.txt").is_file());
    assert!(run.out.join("strips/scene-strip-smoke/frame-000.txt").is_file());
}

#[test]
fn dev_preview_html_contains_paused_strip_controls() {
    let run = PreviewRun::new();

    run.run_success("animation");

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    assert!(html.contains("data-strip-id=\"scene-strip-smoke\""));
    assert!(html.contains("data-strip-play"));
    assert!(html.contains("aria-pressed=\"false\""));
    assert!(html.contains("data-strip-next"));
    assert!(html.contains("data-strip-prev"));
    assert!(!html.contains("https://"));
    assert!(!html.contains("http://"));
}
```

- [ ] **Step 2: Run tests and confirm the expected failures**

Run:

```bash
cargo test --test dev_preview dev_preview_animation_writes_scene_strip_manifest_and_frames
cargo test --test dev_preview dev_preview_all_includes_scene_strips
cargo test --test dev_preview dev_preview_html_contains_paused_strip_controls
```

Expected: `invalid value 'animation' for '--scenario <SCENARIO>'` or missing `strips` fields.

- [ ] **Step 3: Add strip export types**

In `src/dev_preview/export.rs`, add `strips` to `PreviewManifest` after `scenarios`, add `PreviewStrip` types after `PreviewScenario`, and add strip artifact kind support:

```rust
pub struct PreviewManifest {
    pub schema_version: u32,
    pub producer: &'static str,
    pub glorp_version: &'static str,
    pub generated_at: String,
    pub scenarios: Vec<PreviewScenario>,
    pub strips: Vec<PreviewStrip>,
    pub artifacts: Vec<PreviewArtifact>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStrip {
    pub id: String,
    pub kind: PreviewStripKind,
    pub title: String,
    pub intent: String,
    pub dimensions: PreviewDimensions,
    pub target_id: String,
    pub playback: PreviewPlayback,
    pub inputs: BTreeMap<String, Value>,
    pub frames: Vec<PreviewStripFrame>,
    pub review_prompts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PreviewStripKind {
    SceneMoment,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewPlayback {
    pub starts_paused: bool,
    pub frame_duration_ms: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStripFrame {
    pub index: u16,
    pub phase: String,
    pub elapsed_ms: u16,
    pub files: PreviewStripFrameFiles,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewStripFrameFiles {
    pub text: PathBuf,
    pub cells: PathBuf,
}
```

Extend `ArtifactType`:

```rust
#[serde(rename_all = "kebab-case")]
pub enum ArtifactType {
    Text,
    Cells,
    Layout,
    Html,
    Review,
    Asset,
}
```

Keep `Text` and `Cells` as the artifact types for strip frames.

- [ ] **Step 4: Render strips in review markdown and HTML**

Change `write_review_markdown` to add a `## Animation Strips` section after scenarios:

```rust
    if !manifest.strips.is_empty() {
        markdown.push_str("## Animation Strips\n\n");
        for strip in &manifest.strips {
            markdown.push_str(&format!(
                "### {}\n\n{}\n\n",
                strip.title, strip.intent
            ));
            markdown.push_str(&format!(
                "- ID: `{}`\n- Kind: `scene-moment`\n- Target: `{}`\n- Size: `{}x{}`\n- Frames: `{}`\n\n",
                strip.id,
                strip.target_id,
                strip.dimensions.width,
                strip.dimensions.height,
                strip.frames.len()
            ));
            markdown.push_str("Review prompts:\n");
            for prompt in &strip.review_prompts {
                markdown.push_str(&format!("- {prompt}\n"));
            }
            markdown.push('\n');
        }
    }
```

Change `write_index_html` signature to accept strips:

```rust
pub fn write_index_html(
    path: &Path,
    frames: &[PreviewFrame],
    strips: &[PreviewStripBundle],
    generated_at: &str,
) -> Result<()>
```

Render strip HTML with the first frame visible:

```rust
fn render_strip_html(strip: &PreviewStripBundle) -> String {
    let mut html = String::new();
    html.push_str(&format!(
        r#"<article class="strip" data-strip-id="{}" data-frame-index="0" data-frame-count="{}" data-frame-duration="{}">"#,
        escape_html(&strip.manifest.id),
        strip.frames.len(),
        strip.manifest.playback.frame_duration_ms
    ));
    html.push_str(&format!("<h2>{}</h2>", escape_html(&strip.manifest.title)));
    html.push_str(r#"<div class="strip-controls">"#);
    html.push_str(r#"<button type="button" data-strip-prev>Prev</button>"#);
    html.push_str(r#"<button type="button" data-strip-play aria-pressed="false">Play</button>"#);
    html.push_str(r#"<button type="button" data-strip-next>Next</button>"#);
    html.push_str(r#"</div>"#);
    for frame in &strip.frames {
        html.push_str(&format!(
            r#"<div class="strip-frame" data-strip-frame="{}"{}>"#,
            frame.id,
            if frame.id.ends_with("frame-000") { "" } else { " hidden" }
        ));
        html.push_str(&render_grid_html(frame));
        html.push_str("</div>");
    }
    html.push_str("</article>");
    html
}
```

Extract the grid part of `render_frame_html` into `render_grid_html(frame: &PreviewFrame) -> String` and call it from both frame and strip rendering.

- [ ] **Step 5: Add `PreviewStripBundle` and smoke strip frames**

Create `src/dev_preview/strips.rs`:

```rust
use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles,
    PreviewStripKind,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use ratatui::{buffer::Buffer, layout::Rect, style::{Color, Style}};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PreviewStripBundle {
    pub manifest: PreviewStrip,
    pub frames: Vec<PreviewFrame>,
}

pub fn scene_strip_smoke() -> PreviewStripBundle {
    let phases = [("start", 0_u16, "."), ("mid", 350_u16, "*"), ("end", 700_u16, "·")];
    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();

    for (index, (phase, elapsed_ms, glyph)) in phases.into_iter().enumerate() {
        let frame_id = format!("scene-strip-smoke-frame-{index:03}");
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));
        for x in 4..36 {
            buffer[(x, 4)].set_symbol(glyph).set_style(Style::default().fg(Color::Yellow));
        }
        let mut frame = frame_from_buffer(frame_id, format!("Scene Strip Smoke {phase}"), &buffer);
        frame.layout = None;
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: index as u16,
            phase: phase.to_string(),
            elapsed_ms,
            files: PreviewStripFrameFiles {
                text: PathBuf::from(format!("strips/scene-strip-smoke/frame-{index:03}.txt")),
                cells: PathBuf::from(format!(
                    "strips/scene-strip-smoke/frame-{index:03}.cells.json"
                )),
            },
        });
    }

    PreviewStripBundle {
        manifest: PreviewStrip {
            id: "scene-strip-smoke".to_string(),
            kind: PreviewStripKind::SceneMoment,
            title: "Scene Strip Smoke".to_string(),
            intent: "Proves Preview Lab can export and play deterministic scene strips.".to_string(),
            dimensions: PreviewDimensions { width: 40, height: 8 },
            target_id: "watch.room.effect".to_string(),
            playback: PreviewPlayback {
                starts_paused: true,
                frame_duration_ms: 160,
            },
            inputs: BTreeMap::from([
                ("fixture".to_string(), Value::String("strip-smoke".to_string())),
                ("elapsed_ms".to_string(), json!([0, 350, 700])),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Confirm playback starts paused.".to_string(),
                "Step through start, mid, and end frames.".to_string(),
            ],
        },
        frames,
    }
}
```

Add `pub mod strips;` to `src/dev_preview/mod.rs`.

- [ ] **Step 6: Wire scenario selection and writing**

Add `Animation` to `PreviewScenarioArg` in `src/cli.rs` and `PreviewSelection` in `src/dev_preview/scenarios.rs`.

In `src/commands/dev_preview.rs`, map it:

```rust
PreviewScenarioArg::Animation => PreviewSelection::Animation,
```

In `generate_preview_bundle`, create `strips_dir`, build `let mut strips = Vec::new();`, and route:

```rust
PreviewSelection::All => {
    frames.extend(watch_frames(&ctx, &scratch_dir)?);
    frames.extend(habitat_prop_frames(&ctx, &scratch_dir)?);
    frames.extend(pet_frames(&ctx)?);
    strips.push(crate::dev_preview::strips::scene_strip_smoke());
}
PreviewSelection::Animation => {
    strips.push(crate::dev_preview::strips::scene_strip_smoke());
}
```

Write strip frames:

```rust
for strip in &strips {
    let strip_dir = staging_dir.join("strips").join(&strip.manifest.id);
    fs::create_dir_all(&strip_dir)?;
    for (index, frame) in strip.frames.iter().enumerate() {
        write_text_frame(&strip_dir.join(format!("frame-{index:03}.txt")), frame)?;
        write_cells_json(&strip_dir.join(format!("frame-{index:03}.cells.json")), frame)?;
    }
}
```

Populate manifest:

```rust
let manifest = PreviewManifest {
    schema_version: SCHEMA_VERSION,
    producer: PRODUCER,
    glorp_version: env!("CARGO_PKG_VERSION"),
    generated_at,
    scenarios,
    strips: strips.iter().map(|strip| strip.manifest.clone()).collect(),
    artifacts: artifacts_for_frames(&frames)
        .into_iter()
        .chain(artifacts_for_strips(&strips))
        .collect(),
};
```

- [ ] **Step 7: Add paused playback JavaScript**

Append this to `src/dev_preview/assets/preview.js` inside the IIFE:

```javascript
  const stripTimers = new Map();

  const showStripFrame = (strip, index) => {
    const frames = Array.from(strip.querySelectorAll("[data-strip-frame]"));
    const count = frames.length;
    const nextIndex = ((index % count) + count) % count;
    strip.dataset.frameIndex = String(nextIndex);
    frames.forEach((frame, frameIndex) => {
      frame.hidden = frameIndex !== nextIndex;
    });
  };

  const pauseStrip = (strip) => {
    const timer = stripTimers.get(strip);
    if (timer) {
      window.clearInterval(timer);
      stripTimers.delete(strip);
    }
    const play = strip.querySelector("[data-strip-play]");
    if (play) {
      play.setAttribute("aria-pressed", "false");
      play.textContent = "Play";
    }
  };

  for (const strip of document.querySelectorAll("[data-strip-id]")) {
    const play = strip.querySelector("[data-strip-play]");
    const prev = strip.querySelector("[data-strip-prev]");
    const next = strip.querySelector("[data-strip-next]");
    showStripFrame(strip, 0);

    play?.addEventListener("click", () => {
      const pressed = play.getAttribute("aria-pressed") === "true";
      if (pressed) {
        pauseStrip(strip);
        return;
      }
      play.setAttribute("aria-pressed", "true");
      play.textContent = "Pause";
      const duration = Number(strip.dataset.frameDuration || "160");
      const timer = window.setInterval(() => {
        showStripFrame(strip, Number(strip.dataset.frameIndex || "0") + 1);
      }, duration);
      stripTimers.set(strip, timer);
    });

    prev?.addEventListener("click", () => {
      pauseStrip(strip);
      showStripFrame(strip, Number(strip.dataset.frameIndex || "0") - 1);
    });

    next?.addEventListener("click", () => {
      pauseStrip(strip);
      showStripFrame(strip, Number(strip.dataset.frameIndex || "0") + 1);
    });
  }
```

Add CSS:

```css
.strip {
  margin-top: 24px;
}

.strip-controls {
  display: flex;
  gap: 8px;
  margin: 8px 0;
}

.strip-controls button {
  border: 1px solid #5a5148;
  border-radius: 6px;
  padding: 5px 9px;
  background: #211e1a;
  color: #e8e3da;
  font: inherit;
  font-size: 13px;
}

.strip-controls button[aria-pressed="true"] {
  border-color: #d6a657;
  background: #3a2c19;
  color: #ffd88a;
}
```

- [ ] **Step 8: Run and commit**

Run:

```bash
cargo fmt --check
cargo test --test dev_preview dev_preview_animation_writes_scene_strip_manifest_and_frames
cargo test --test dev_preview dev_preview_all_includes_scene_strips
cargo test --test dev_preview dev_preview_html_contains_paused_strip_controls
cargo test dev_preview::export
```

Expected: all pass.

Commit:

```bash
git add src/dev_preview src/cli.rs src/commands/dev_preview.rs tests/dev_preview.rs
git commit -m "feat(preview): add scene animation strips"
```

## Task 2: Add Pure Room Profile Derivation

**Spec sections:** System Overview, Prop Biomes, Prop Emitters, Pet Performance, Tachyonfx Moments.

**Files:**
- Create: `src/tui/room.rs`
- Modify: `src/tui/mod.rs`
- Test: `src/tui/room.rs`

- [ ] **Step 1: Write the failing room-profile tests**

Create `src/tui/room.rs` with the module skeleton and tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::habitat::{
        CODEX_SIGNAL_LAMP, HEAVY_SESSION_PLANTER, TOKEN_LANTERN_10M, TOKEN_MOSS_TUFT_250K,
        TOKEN_ORBIT_5M, TOKEN_SHELL_100K,
    };
    use crate::storage::state::{HabitatPropId, HabitatPropSource};
    use crate::tui::day::{DayContext, DayPhase};
    use crate::tui::life::{IdleLifeState, PetLifeProfile, PropReaction, PropReactionKind, WorkWeather};
    use crate::tui::view_model::{EarnedHabitatPropView, HabitatView, WatchViewModel};
    use time::macros::datetime;

    fn earned(id: &str, priority: i16) -> EarnedHabitatPropView {
        EarnedHabitatPropView {
            id: HabitatPropId::new(id),
            earned_at: datetime!(2026-06-10 12:00 UTC),
            kind: crate::game::habitat::catalog_prop_by_str(id).unwrap().kind,
            display_priority: priority,
            source: HabitatPropSource::LifetimeTokens { threshold: 1.0 },
        }
    }

    fn vm_with_props(props: Vec<EarnedHabitatPropView>) -> WatchViewModel {
        let mut vm = WatchViewModel::fixture();
        vm.habitat = HabitatView { earned_props: props };
        vm.day_context = DayContext {
            day_phase: DayPhase::Day,
            mature: true,
            ..DayContext::default()
        };
        vm
    }

    #[test]
    fn biome_uses_all_earned_props_not_visible_rotation_only() {
        let vm = vm_with_props(vec![
            earned(HEAVY_SESSION_PLANTER, 80),
            earned(TOKEN_MOSS_TUFT_250K, 25),
            earned(CODEX_SIGNAL_LAMP, 70),
            earned(TOKEN_ORBIT_5M, 50),
            earned(TOKEN_SHELL_100K, 20),
        ]);

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        assert_eq!(profile.biome.primary, RoomBiomeTag::Botanical);
        assert!(profile.biome.secondary.is_some());
        assert!(
            profile.identity_prop_ids.contains(&HabitatPropId::from(HEAVY_SESSION_PLANTER)),
            "high-weight earned props should anchor the room identity"
        );
    }

    #[test]
    fn starter_room_has_no_emitter_or_scene_moments() {
        let vm = vm_with_props(Vec::new());

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        assert_eq!(profile.biome.primary, RoomBiomeTag::Starter);
        assert_eq!(profile.resonant_emitter, None);
        assert!(profile.scene_moments.is_empty());
    }

    #[test]
    fn live_prop_reaction_selects_visible_emitter() {
        let mut vm = vm_with_props(vec![earned(CODEX_SIGNAL_LAMP, 70), earned(TOKEN_LANTERN_10M, 60)]);
        vm.life_profile = PetLifeProfile {
            activity_level: 1.2,
            burst_level: 0.9,
            work_weather: WorkWeather::OutputSparks,
            prop_reactions: vec![PropReaction {
                prop_id: HabitatPropId::from(CODEX_SIGNAL_LAMP),
                intensity: 0.8,
                kind: PropReactionKind::Glow,
            }],
            idle: IdleLifeState {
                idle_minutes: 0,
                is_recently_active: true,
            },
            ..PetLifeProfile::default()
        };

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

        assert_eq!(
            profile.resonant_emitter.as_ref().map(|emitter| emitter.prop_id.clone()),
            Some(HabitatPropId::from(CODEX_SIGNAL_LAMP))
        );
        assert!(
            profile
                .scene_moments
                .iter()
                .any(|moment| moment.key == SceneMomentKey::FeedSweep)
        );
    }

    #[test]
    fn pet_performance_sleep_beats_live_burst() {
        let mut vm = vm_with_props(vec![earned(TOKEN_LANTERN_10M, 60)]);
        vm.day_context = DayContext {
            asleep: true,
            mature: true,
            ..vm.day_context
        };
        vm.life_profile.burst_level = 1.0;

        let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 03:00 UTC));

        assert_eq!(profile.pet_performance, PetPerformance::AsleepDreaming);
        assert!(
            !profile
                .scene_moments
                .iter()
                .any(|moment| moment.key == SceneMomentKey::FeedSweep),
            "sleeping room should not fake a live feed burst"
        );
    }
}
```

- [ ] **Step 2: Run tests and confirm unresolved types**

Run:

```bash
cargo test tui::room --lib
```

Expected: unresolved `RoomBiomeTag`, `derive_room_life_profile`, `PetPerformance`, and `SceneMomentKey`.

- [ ] **Step 3: Implement room profile types and derivation**

Add the production code above the tests in `src/tui/room.rs`:

```rust
use crate::game::habitat::{
    catalog_prop_by_str, CODEX_SIGNAL_LAMP, HEAVY_SESSION_PLANTER, TOKEN_FRIENDLY_CLOUD_750K,
    TOKEN_HANGING_VINE_25M, TOKEN_LANTERN_10M, TOKEN_MOSS_TUFT_250K, TOKEN_ORBIT_5M,
    TOKEN_PEBBLE_25K, TOKEN_SHELL_100K, TOKEN_SHARD_1M, TOKEN_SPARK_500K,
    TOKEN_TREASURE_CHEST_2M, WILT_RECOVERY_SPROUT,
};
use crate::storage::state::HabitatPropId;
use crate::tui::day::{in_morning_after_window, resonant_prop_for_day, DayPhase};
use crate::tui::life::{PetLifeProfile, PropReactionKind, WorkWeather};
use crate::tui::view_model::{EarnedHabitatPropView, WatchViewModel};
use time::{Duration, OffsetDateTime};

const BASE_EARNED_PROP_WEIGHT: f32 = 1.0;
const RECENT_EARNED_BONUS: f32 = 0.4;
const RESONANT_BONUS: f32 = 1.2;
const SECONDARY_THRESHOLD: f32 = 0.6;

#[derive(Debug, Clone, PartialEq)]
pub struct RoomLifeProfile {
    pub biome: RoomBiome,
    pub room_weather: RoomWeatherLayer,
    pub resonant_emitter: Option<PropEmitter>,
    pub pet_performance: PetPerformance,
    pub scene_moments: Vec<SceneMoment>,
    pub identity_prop_ids: Vec<HabitatPropId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomBiomeTag {
    Starter,
    Botanical,
    Technical,
    Celestial,
    Artifact,
    Cozy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoomBiome {
    pub primary: RoomBiomeTag,
    pub secondary: Option<RoomBiomeTag>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomWeatherLayer {
    Clear,
    CacheMist,
    OutputSparks,
    ReasoningPulse,
    Mixed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropEmitter {
    pub prop_id: HabitatPropId,
    pub behavior: PropEmitterBehavior,
    pub intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropEmitterBehavior {
    LeafDrift,
    TechnicalPing,
    HopefulSprout,
    WarmHalo,
    CloudDrift,
    OrbitArc,
    ArtifactGlint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetPerformance {
    RestedAwake,
    TiredAwake,
    HeavyDayCozy,
    AsleepDreaming,
    CatchUpWake,
    SourceBurstPerk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneMoment {
    pub key: SceneMomentKey,
    pub trigger_id: SceneTriggerId,
    pub target_id: &'static str,
    pub duration_ms: u16,
    pub max_replay_age_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SceneMomentKey {
    FeedSweep,
    PropResonanceRipple,
    DawnWakeWipe,
    HeavySessionShimmer,
    DreamGlimmer,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SceneTriggerId(String);

impl SceneTriggerId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

pub fn derive_room_life_profile(vm: &WatchViewModel, now: OffsetDateTime) -> RoomLifeProfile {
    let resonant = resonant_prop_from_vm(vm);
    let biome = derive_biome(&vm.habitat.earned_props, resonant.as_ref(), now);
    let room_weather = room_weather_layer(vm.life_profile.work_weather);
    let pet_performance = pet_performance_for(vm, now);
    let visible_prop_ids = visible_identity_ids(&vm.habitat.earned_props);
    let resonant_emitter = select_emitter(vm, resonant.as_ref(), room_weather, &visible_prop_ids);
    let scene_moments = scene_moments_for(vm, now, resonant_emitter.as_ref(), pet_performance);

    RoomLifeProfile {
        biome,
        room_weather,
        resonant_emitter,
        pet_performance,
        scene_moments,
        identity_prop_ids: visible_prop_ids,
    }
}
```

Add helper functions in the same file. Use `match` tables for prop tags and emitter behaviors so implementation is deterministic:

```rust
fn tags_for_prop(id: &str) -> &'static [RoomBiomeTag] {
    match id {
        TOKEN_MOSS_TUFT_250K | TOKEN_HANGING_VINE_25M | HEAVY_SESSION_PLANTER
        | WILT_RECOVERY_SPROUT => &[RoomBiomeTag::Botanical, RoomBiomeTag::Cozy],
        CODEX_SIGNAL_LAMP => &[RoomBiomeTag::Technical],
        TOKEN_ORBIT_5M => &[RoomBiomeTag::Technical, RoomBiomeTag::Celestial],
        TOKEN_SPARK_500K | TOKEN_FRIENDLY_CLOUD_750K | TOKEN_LANTERN_10M => {
            &[RoomBiomeTag::Celestial, RoomBiomeTag::Cozy]
        }
        TOKEN_PEBBLE_25K | TOKEN_SHELL_100K | TOKEN_SHARD_1M | TOKEN_TREASURE_CHEST_2M => {
            &[RoomBiomeTag::Artifact]
        }
        _ => &[],
    }
}

fn emitter_behavior_for_prop(id: &str) -> PropEmitterBehavior {
    match id {
        HEAVY_SESSION_PLANTER => PropEmitterBehavior::LeafDrift,
        CODEX_SIGNAL_LAMP => PropEmitterBehavior::TechnicalPing,
        WILT_RECOVERY_SPROUT => PropEmitterBehavior::HopefulSprout,
        TOKEN_LANTERN_10M => PropEmitterBehavior::WarmHalo,
        TOKEN_FRIENDLY_CLOUD_750K => PropEmitterBehavior::CloudDrift,
        TOKEN_ORBIT_5M => PropEmitterBehavior::OrbitArc,
        _ => PropEmitterBehavior::ArtifactGlint,
    }
}
```

Make `scene_moments_for` create stable trigger ids:

```rust
fn scene_moments_for(
    vm: &WatchViewModel,
    now: OffsetDateTime,
    emitter: Option<&PropEmitter>,
    performance: PetPerformance,
) -> Vec<SceneMoment> {
    let mut moments = Vec::new();
    if vm.life_profile.burst_level > 0.0
        && !vm.day_context.asleep
        && vm.last_feed_pulse_at.is_some_and(|pulse| now - pulse <= Duration::seconds(8))
    {
        moments.push(SceneMoment {
            key: SceneMomentKey::FeedSweep,
            trigger_id: SceneTriggerId::new(format!(
                "feed:{}",
                vm.last_feed_pulse_at.unwrap().unix_timestamp()
            )),
            target_id: "watch.pet.effect",
            duration_ms: 500,
            max_replay_age_ms: 8_000,
        });
    }
    if let Some(emitter) = emitter {
        moments.push(SceneMoment {
            key: SceneMomentKey::PropResonanceRipple,
            trigger_id: SceneTriggerId::new(format!(
                "prop:{}:{}",
                emitter.prop_id.as_str(),
                vm.day_context.date_seed
            )),
            target_id: prop_target_id(emitter.prop_id.as_str()),
            duration_ms: 700,
            max_replay_age_ms: 3_600_000,
        });
    }
    if in_morning_after_window(&vm.day_context, now) && matches!(performance, PetPerformance::CatchUpWake) {
        moments.push(SceneMoment {
            key: SceneMomentKey::DawnWakeWipe,
            trigger_id: SceneTriggerId::new(format!("wake:{}", vm.day_context.date_seed)),
            target_id: "watch.room.effect",
            duration_ms: 900,
            max_replay_age_ms: 3_600_000,
        });
    }
    moments
}
```

- [ ] **Step 4: Expose module and run tests**

Add to `src/tui/mod.rs`:

```rust
pub mod room;
```

Run:

```bash
cargo fmt --check
cargo test tui::room --lib
```

Expected: pass.

Commit:

```bash
git add src/tui/mod.rs src/tui/room.rs
git commit -m "feat(tui): derive alive room profile"
```

## Task 3: Make Prop Placement And Preview Targets Share One Geometry Result

**Spec sections:** Prop Emitters, Tachyonfx Moments, Preview Lab Proof.

**Files:**
- Modify: `src/tui/component/habitat_props.rs`
- Modify: `src/tui/component/geometry.rs`
- Modify: `src/tui/component/pet_scene.rs`
- Modify: `src/tui/component/preview.rs`
- Modify: `src/tui/panels/pet.rs`
- Test: `src/tui/component/habitat_props.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Write failing placement tests**

In `src/tui/component/habitat_props.rs`, add tests:

```rust
#[test]
fn prop_placements_group_cells_with_bounds_and_static_targets() {
    let ctx = RenderContext::with_clock(
        ColorCapability::Truecolor,
        crate::tui::render_context::WatchClock::fixed(time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap()),
    );
    let scene = test_scene();
    let habitat = habitat_with_props(&[
        crate::game::habitat::CODEX_SIGNAL_LAMP,
        crate::game::habitat::HEAVY_SESSION_PLANTER,
    ]);

    let placements = habitat_prop_placements_for(&habitat, &scene, &[], Species::Fuzz, "seed", &ctx);

    assert!(placements.iter().any(|placement| {
        placement.prop_id.as_str() == crate::game::habitat::CODEX_SIGNAL_LAMP
            && placement.target_id.as_ref().unwrap().as_str() == "watch.prop.codex_signal_lamp.effect"
            && !placement.cells.is_empty()
            && placement.bounds.width > 0
            && placement.bounds.height > 0
    }));
}

#[test]
fn habitat_props_for_flattens_shared_placements() {
    let ctx = RenderContext::with_clock(
        ColorCapability::Truecolor,
        crate::tui::render_context::WatchClock::fixed(time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap()),
    );
    let scene = test_scene();
    let habitat = habitat_with_props(&[crate::game::habitat::HEAVY_SESSION_PLANTER]);

    let placements = habitat_prop_placements_for(&habitat, &scene, &[], Species::Fuzz, "seed", &ctx);
    let cells = habitat_props_for(&habitat, &scene, &[], Species::Fuzz, "seed", &ctx);

    assert_eq!(
        cells,
        placements
            .iter()
            .flat_map(|placement| placement.cells.clone())
            .collect::<Vec<_>>()
    );
}
```

- [ ] **Step 2: Run tests and confirm unresolved function**

Run:

```bash
cargo test habitat_prop_placements_for --lib
```

Expected: unresolved `habitat_prop_placements_for`.

- [ ] **Step 3: Implement placement structs and static target ids**

Add beside `HabitatPropCell`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct HabitatPropPlacement {
    pub prop_id: HabitatPropId,
    pub cells: Vec<HabitatPropCell>,
    pub bounds: Rect,
    pub pet_layer: HabitatPetLayer,
    pub target_id: Option<TargetPath>,
}
```

Add static target mapping:

```rust
pub fn prop_effect_target(id: &str) -> Option<TargetPath> {
    match id {
        crate::game::habitat::TOKEN_PEBBLE_25K => Some(TargetPath::new("watch.prop.token_pebble_25k.effect")),
        crate::game::habitat::TOKEN_SHELL_100K => Some(TargetPath::new("watch.prop.token_shell_100k.effect")),
        crate::game::habitat::TOKEN_MOSS_TUFT_250K => Some(TargetPath::new("watch.prop.token_moss_tuft_250k.effect")),
        crate::game::habitat::TOKEN_SPARK_500K => Some(TargetPath::new("watch.prop.token_spark_500k.effect")),
        crate::game::habitat::TOKEN_FRIENDLY_CLOUD_750K => Some(TargetPath::new("watch.prop.token_friendly_cloud_750k.effect")),
        crate::game::habitat::TOKEN_SHARD_1M => Some(TargetPath::new("watch.prop.token_shard_1m.effect")),
        crate::game::habitat::TOKEN_TREASURE_CHEST_2M => Some(TargetPath::new("watch.prop.token_treasure_chest_2m.effect")),
        crate::game::habitat::TOKEN_ORBIT_5M => Some(TargetPath::new("watch.prop.token_orbit_5m.effect")),
        crate::game::habitat::TOKEN_LANTERN_10M => Some(TargetPath::new("watch.prop.token_lantern_10m.effect")),
        crate::game::habitat::TOKEN_HANGING_VINE_25M => Some(TargetPath::new("watch.prop.token_hanging_vine_25m.effect")),
        crate::game::habitat::CODEX_SIGNAL_LAMP => Some(TargetPath::new("watch.prop.codex_signal_lamp.effect")),
        crate::game::habitat::HEAVY_SESSION_PLANTER => Some(TargetPath::new("watch.prop.heavy_session_planter.effect")),
        crate::game::habitat::WILT_RECOVERY_SPROUT => Some(TargetPath::new("watch.prop.wilt_recovery_sprout.effect")),
        _ => None,
    }
}
```

Refactor `habitat_props_for` into:

```rust
pub fn habitat_props_for(
    habitat: &HabitatView,
    scene: &PetSceneLayout,
    silhouette_halo: &[Rect],
    species: Species,
    seed: &str,
    ctx: &RenderContext,
) -> Vec<HabitatPropCell> {
    habitat_prop_placements_for(habitat, scene, silhouette_halo, species, seed, ctx)
        .into_iter()
        .flat_map(|placement| placement.cells)
        .collect()
}
```

Create `habitat_prop_placements_for` by moving the existing placement loops and pushing `HabitatPropPlacement` each time a rendered sprite or accent cell is selected. For one-cell accents, `bounds` is `Rect::new(cell.col, cell.row, 1, 1)`.

- [ ] **Step 4: Add room and prop target roles**

In `src/tui/component/geometry.rs`, extend `TargetRole`:

```rust
RoomEffect,
PropEffect,
```

In `src/tui/component/pet_scene.rs`, add a room effect target with `scene.habitat`:

```rust
insert_target(
    &mut targets,
    TargetPath::new("watch.room.effect"),
    owner,
    habitat,
    5,
    area,
    TargetRole::RoomEffect,
);
```

Append it to `effect_targets`.

- [ ] **Step 5: Upgrade PreviewLayout targets without breaking x/y callers**

In `src/tui/component/preview.rs`, replace `targets: BTreeMap<String, PreviewRect>` with:

```rust
pub targets: BTreeMap<String, PreviewTarget>,
```

Add:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTarget {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub owner: String,
    pub role: String,
    pub clip: PreviewRect,
    pub z: i16,
    pub layer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_count: Option<usize>,
}
```

Map component targets with:

```rust
fn target(path: &TargetPath, target: &GeometryTarget) -> PreviewTarget {
    PreviewTarget {
        x: target.rect.x,
        y: target.rect.y,
        width: target.rect.width,
        height: target.rect.height,
        owner: target.owner.as_str().to_string(),
        role: format!("{:?}", target.role),
        clip: rect(target.clip),
        z: target.z,
        layer: if path.as_str() == "watch.room.effect" {
            "room-background".to_string()
        } else if path.as_str().starts_with("watch.prop.") {
            "prop".to_string()
        } else {
            "component".to_string()
        },
        cell_count: None,
    }
}
```

Keep `x`, `y`, `width`, and `height` top-level so existing `tests/dev_preview.rs::cells_for_target` continues to work.

- [ ] **Step 6: Add preview integration assertions**

In `tests/dev_preview.rs`, add:

```rust
#[test]
fn dev_preview_layout_targets_include_owner_role_clip_and_layer() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let layout = read_layout(&run, "watch-wide-normal");
    let room = &layout["targets"]["watch.room.effect"];
    assert_eq!(room["owner"], "watch.pet");
    assert_eq!(room["role"], "RoomEffect");
    assert_eq!(room["layer"], "room-background");
    assert!(room["clip"].is_object());

    let pet = &layout["targets"]["watch.pet.effect"];
    assert_eq!(pet["role"], "Effect");
    assert_eq!(pet["layer"], "component");
}
```

- [ ] **Step 7: Run and commit**

Run:

```bash
cargo fmt --check
cargo test habitat_prop_placements_for --lib
cargo test --test dev_preview dev_preview_layout_targets_include_owner_role_clip_and_layer
cargo test --test dev_preview dev_preview_watch_writes_layout_artifacts_and_manifest_entries
```

Expected: all pass.

Commit:

```bash
git add src/tui/component src/tui/panels/pet.rs tests/dev_preview.rs
git commit -m "feat(tui): expose room and prop effect targets"
```

## Task 4: Render Resting Biomes, Weather, And Emitters

**Spec sections:** Prop Biomes, Prop Emitters, Weather And Day Overlays, Success Criteria.

**Files:**
- Modify: `src/tui/room.rs`
- Modify: `src/tui/panels/pet.rs`
- Modify: `tests/tui_render.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add low-color and zone-diff tests**

In `tests/dev_preview.rs`, add helpers:

```rust
fn changed_cells_by_symbol(a: &[Value], b: &[Value]) -> usize {
    assert_eq!(a.len(), b.len());
    a.iter().zip(b).filter(|(left, right)| left["symbol"] != right["symbol"]).count()
}

fn changed_room_zones(a: &[Value], b: &[Value], width: u64, height: u64) -> std::collections::BTreeSet<&'static str> {
    let mut zones = std::collections::BTreeSet::new();
    for (left, right) in a.iter().zip(b) {
        if left["symbol"] == right["symbol"] {
            continue;
        }
        let x = left["x"].as_u64().unwrap();
        let y = left["y"].as_u64().unwrap();
        let zone = if y < height / 3 {
            "upper-air"
        } else if y > height * 2 / 3 {
            "floor"
        } else if x < width / 3 {
            "left-anchor"
        } else if x > width * 2 / 3 {
            "right-anchor"
        } else {
            "pet-adjacent"
        };
        zones.insert(zone);
    }
    zones
}
```

Add the failing test:

```rust
#[test]
fn alive_room_fixtures_differ_by_symbols_in_multiple_room_zones() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let botanical_cells = read_cells(&run, "room-botanical-cache-evening");
    let botanical_layout = read_layout(&run, "room-botanical-cache-evening");
    let technical_cells = read_cells(&run, "room-technical-output-active");
    let technical_layout = read_layout(&run, "room-technical-output-active");

    let botanical_room = cells_for_target(&botanical_cells, &botanical_layout, "watch.room.effect");
    let technical_room = cells_for_target(&technical_cells, &technical_layout, "watch.room.effect");
    let changed = changed_cells_by_symbol(&botanical_room, &technical_room);
    let rect = &botanical_layout["targets"]["watch.room.effect"];
    let zones = changed_room_zones(
        &botanical_room,
        &technical_room,
        rect["width"].as_u64().unwrap(),
        rect["height"].as_u64().unwrap(),
    );

    assert!(changed >= 24, "room states should differ by symbols; changed {changed}");
    assert!(zones.len() >= 2, "room states should differ across zones; got {zones:?}");
}
```

- [ ] **Step 2: Run and confirm missing fixtures**

Run:

```bash
cargo test --test dev_preview alive_room_fixtures_differ_by_symbols_in_multiple_room_zones
```

Expected: missing scenario `room-botanical-cache-evening`.

- [ ] **Step 3: Add room glyph primitives**

In `src/tui/room.rs`, add:

```rust
use ratatui::{layout::Rect, style::{Color, Style}};

#[derive(Debug, Clone, PartialEq)]
pub struct RoomGlyph {
    pub row: u16,
    pub col: u16,
    pub glyph: char,
    pub style: Style,
    pub zone: RoomZone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RoomZone {
    Floor,
    UpperAir,
    LeftAnchor,
    RightAnchor,
    PetAdjacent,
}

pub fn room_glyphs_for(
    profile: &RoomLifeProfile,
    area: Rect,
    exclusions: &[Rect],
    now: OffsetDateTime,
    color_capability: crate::tui::style::ColorCapability,
) -> Vec<RoomGlyph> {
    let mut glyphs = biome_glyphs(profile, area, now, color_capability);
    glyphs.extend(weather_glyphs(profile, area, now, color_capability));
    glyphs.extend(emitter_glyphs(profile, area, now, color_capability));
    glyphs
        .into_iter()
        .filter(|glyph| !rects_contain(exclusions, glyph.col, glyph.row))
        .take(motion_budget(area))
        .collect()
}

pub fn motion_budget(area: Rect) -> usize {
    if area.width <= 72 || area.height <= 24 {
        8
    } else if area.width >= 180 || area.height >= 50 {
        28
    } else {
        16
    }
}
```

Use these biome symbol families:

```rust
fn biome_symbols(tag: RoomBiomeTag) -> &'static [char] {
    match tag {
        RoomBiomeTag::Starter => &['.', '·'],
        RoomBiomeTag::Botanical => &['"', '\'', '`', ','],
        RoomBiomeTag::Technical => &[':', ';', '+', '='],
        RoomBiomeTag::Celestial => &['*', '·', '˚', '.'],
        RoomBiomeTag::Artifact => &['.', 'o', '◇', '°'],
        RoomBiomeTag::Cozy => &['~', '·', '⌞', '⌟'],
    }
}
```

When `ColorCapability::Flat`, still use the symbol families and skip RGB-specific styling.

Add budget tests in `src/tui/room.rs`:

```rust
#[test]
fn room_motion_budget_matches_preview_size_classes() {
    assert_eq!(motion_budget(Rect::new(0, 0, 72, 24)), 8);
    assert_eq!(motion_budget(Rect::new(0, 0, 120, 32)), 16);
    assert_eq!(motion_budget(Rect::new(0, 0, 180, 50)), 28);
}
```

- [ ] **Step 4: Render room glyphs before props**

In `src/tui/panels/pet.rs`, inside `PetPanel::render` after `ambient_exclusions` is built and before existing ambient glyph rendering, derive the room profile and paint room glyphs:

```rust
let room_profile = crate::tui::room::derive_room_life_profile(vm, now);
let room_glyphs = crate::tui::room::room_glyphs_for(
    &room_profile,
    scene.habitat,
    &ambient_exclusions,
    now,
    ctx.color_capability,
);
for g in room_glyphs {
    if ambient_glyph_is_inside_area(
        &AmbientGlyph {
            row: g.row,
            col: g.col,
            glyph: g.glyph,
            color: g.style.fg.unwrap_or(tokenpet_palette().faint.rgb),
        },
        scene.habitat,
    ) {
        let cell = &mut buf[(g.col, g.row)];
        cell.set_char(g.glyph);
        cell.set_style(g.style);
    }
}
```

Keep existing ambient/mote/activity passes after this. This preserves current behavior while adding a stronger room base.

- [ ] **Step 5: Run and commit**

Run:

```bash
cargo fmt --check
cargo test tui::room --lib
cargo test --test dev_preview alive_room_fixtures_differ_by_symbols_in_multiple_room_zones
cargo test --test dev_preview dev_preview_liveliness_changes_pet_scene_cells_not_only_text
```

Expected: all pass after Task 6 adds fixtures; if run before Task 6, the first command passes and the fixture test keeps failing at missing fixture. Do not commit this task until the room render test has a real passing fixture.

Commit after Task 6 if both tasks land together:

```bash
git add src/tui/room.rs src/tui/panels/pet.rs tests/dev_preview.rs
git commit -m "feat(tui): render alive room biomes"
```

## Task 5: Add Pet Performance Cues

**Spec sections:** Pet Performance, Preview Lab Proof.

**Files:**
- Modify: `src/tui/room.rs`
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/pet/animator.rs`
- Test: `src/tui/panels/pet.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Add performance selection tests**

In `src/tui/room.rs`, add:

```rust
#[test]
fn pet_performance_tired_and_heavy_day_are_distinct() {
    let mut vm = vm_with_props(vec![earned(HEAVY_SESSION_PLANTER, 80)]);
    vm.day_context = DayContext {
        mature: true,
        tiredness: 0.8,
        today_ratio: 1.4,
        day_phase: DayPhase::Dusk,
        ..vm.day_context
    };

    let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 19:00 UTC));

    assert_eq!(profile.pet_performance, PetPerformance::HeavyDayCozy);
}

#[test]
fn source_burst_perk_requires_live_burst() {
    let mut vm = vm_with_props(vec![earned(CODEX_SIGNAL_LAMP, 70)]);
    vm.life_profile.burst_level = 0.9;
    vm.last_feed_pulse_at = Some(datetime!(2026-06-11 10:00 UTC));

    let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00:02 UTC));

    assert_eq!(profile.pet_performance, PetPerformance::SourceBurstPerk);
}
```

- [ ] **Step 2: Add render helpers for cues**

In `src/tui/panels/pet.rs`, add small helpers near `render_pet_inside`:

```rust
fn apply_pet_performance_cues(
    buf: &mut Buffer,
    scene: &PetSceneLayout,
    performance: crate::tui::room::PetPerformance,
    color_capability: ColorCapability,
) {
    if matches!(color_capability, ColorCapability::Flat) {
        apply_flat_performance_cues(buf, scene, performance);
        return;
    }
    match performance {
        crate::tui::room::PetPerformance::TiredAwake => mark_pet_floor(buf, scene, '˙'),
        crate::tui::room::PetPerformance::HeavyDayCozy => mark_pet_floor(buf, scene, '~'),
        crate::tui::room::PetPerformance::AsleepDreaming => mark_pet_air(buf, scene, 'z'),
        crate::tui::room::PetPerformance::CatchUpWake => mark_pet_air(buf, scene, '^'),
        crate::tui::room::PetPerformance::SourceBurstPerk => mark_pet_air(buf, scene, '!'),
        crate::tui::room::PetPerformance::RestedAwake => {}
    }
}
```

Call it after `render_pet_inside` and before foreground props:

```rust
apply_pet_performance_cues(
    buf,
    &scene,
    room_profile.pet_performance,
    ctx.color_capability,
);
```

Keep cues tiny: one or two cells near the pet, never a template rewrite.

- [ ] **Step 3: Verify cue previews**

Add a dev-preview assertion after Task 6 creates fixtures:

```rust
#[test]
fn alive_room_pet_performance_fixtures_change_pet_adjacent_symbols() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let heavy = read_cells(&run, "room-heavy-day-cozy-large");
    let heavy_layout = read_layout(&run, "room-heavy-day-cozy-large");
    let dawn = read_cells(&run, "room-dawn-wake-small");
    let dawn_layout = read_layout(&run, "room-dawn-wake-small");
    let heavy_pet = cells_for_target(&heavy, &heavy_layout, "watch.pet.art");
    let dawn_pet = cells_for_target(&dawn, &dawn_layout, "watch.pet.art");

    assert!(
        changed_cells_by_symbol(&heavy_pet, &dawn_pet) >= 2,
        "pet performance fixtures should produce readable pet-local differences"
    );
}
```

- [ ] **Step 4: Run and commit with Task 6 fixtures**

Run:

```bash
cargo fmt --check
cargo test tui::room --lib
cargo test --test dev_preview alive_room_pet_performance_fixtures_change_pet_adjacent_symbols
```

Expected: pass after Task 6 supplies fixture IDs.

Commit:

```bash
git add src/tui/room.rs src/tui/panels/pet.rs tests/dev_preview.rs
git commit -m "feat(tui): add alive room pet performance cues"
```

## Task 6: Add Alive Room Preview Fixtures And Cropped Review Contract

**Spec sections:** Required Preview Fixtures, Automated visual acceptance, Success Criteria.

**Files:**
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/export.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add expected fixture ids**

In `tests/dev_preview.rs`, add:

```rust
const ALIVE_ROOM_WATCH_IDS: [&str; 8] = [
    "room-starter-day-clear",
    "room-botanical-cache-evening",
    "room-technical-output-active",
    "room-celestial-artifact-night",
    "room-cozy-weekend-quiet",
    "room-mixed-full-wide",
    "room-heavy-day-cozy-large",
    "room-dawn-wake-small",
];
```

Extend `dev_preview_watch_writes_expected_artifacts` and `dev_preview_all_writes_watch_and_pet_artifacts` to assert text/cells/layout files for each id.

Add manifest assertions:

```rust
#[test]
fn dev_preview_alive_room_fixtures_include_room_profile_inputs() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in ALIVE_ROOM_WATCH_IDS {
        let scenario = scenario(&manifest, id);
        assert!(scenario["inputs"]["room_life_profile"].is_object(), "{id} missing room_life_profile");
        assert!(scenario["inputs"]["expected_room_life_profile"].is_object(), "{id} missing expected profile");
        assert!(scenario["review_prompts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|prompt| prompt.as_str().unwrap().contains("primary biome")));
    }
}
```

- [ ] **Step 2: Add fixture data in `src/dev_preview/watch.rs`**

Add an `AliveRoomFrameFixture` struct near the existing fixture structs:

```rust
struct AliveRoomFrameFixture {
    id: &'static str,
    title: &'static str,
    width: u16,
    height: u16,
    species: Species,
    stage: Stage,
    props: &'static [&'static str],
    profile: PetLifeProfile,
    day_context: DayContext,
    expected_biome: &'static str,
    expected_emitter: Option<&'static str>,
}
```

Create `alive_room_frame_fixtures(ctx)` with the eight spec ids. Reuse existing `warm_life_profile`, `hot_life_profile`, `calm_idle_life_profile`, and existing day-context helper patterns; set explicit `work_weather`, `day_phase`, `tiredness`, `asleep`, and `wake_resume` fields per fixture.

Render each fixture through `render_watch_frame_from_state_with_life`, then insert `room_life_profile` and `expected_room_life_profile` in `scenario_metadata`.

Use this JSON shape:

```rust
(
    "expected_room_life_profile".to_string(),
    json!({
        "primary_biome": fixture.expected_biome,
        "emitter": fixture.expected_emitter,
    }),
)
```

For the actual profile:

```rust
let room_profile = crate::tui::room::derive_room_life_profile(&vm, ctx.fixed_now);
(
    "room_life_profile".to_string(),
    json!({
        "primary_biome": format!("{:?}", room_profile.biome.primary),
        "secondary_biome": room_profile.biome.secondary.map(|tag| format!("{tag:?}")),
        "emitter": room_profile.resonant_emitter.as_ref().map(|emitter| emitter.prop_id.as_str().to_string()),
        "pet_performance": format!("{:?}", room_profile.pet_performance),
        "scene_moments": room_profile.scene_moments.iter().map(|moment| format!("{:?}", moment.key)).collect::<Vec<_>>(),
    }),
)
```

- [ ] **Step 3: Add cropped room artifacts**

In `src/dev_preview/export.rs`, add an optional `room_text` field to `PreviewScenarioFiles`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub room_text: Option<PathBuf>,
```

For every watch frame with `watch.room.effect`, write `frames/<id>.room.txt` by cropping cells to that target and preserving line breaks. Add an `ArtifactType::Text` artifact id `<id>-room`.

Add this test:

```rust
#[test]
fn dev_preview_alive_room_writes_cropped_room_artifacts() {
    let run = PreviewRun::new();

    run.run_success("watch");

    for id in ALIVE_ROOM_WATCH_IDS {
        assert!(run.out.join(format!("frames/{id}.room.txt")).is_file(), "missing cropped room for {id}");
        let scenario = scenario(&run.manifest(), id);
        assert_eq!(scenario["files"]["room_text"], format!("frames/{id}.room.txt"));
    }
}
```

- [ ] **Step 4: Run and commit**

Run:

```bash
cargo fmt --check
cargo test --test dev_preview dev_preview_alive_room_fixtures_include_room_profile_inputs
cargo test --test dev_preview dev_preview_alive_room_writes_cropped_room_artifacts
cargo test --test dev_preview alive_room_fixtures_differ_by_symbols_in_multiple_room_zones
cargo test --test dev_preview alive_room_pet_performance_fixtures_change_pet_adjacent_symbols
```

Expected: all pass.

Commit:

```bash
git add src/dev_preview src/tui/room.rs src/tui/panels/pet.rs tests/dev_preview.rs
git commit -m "feat(preview): add alive room review fixtures"
```

## Task 7: Extend The Animator Into Finite Scene Moments

**Spec sections:** Tachyonfx Moments, Testing, Success Criteria.

**Files:**
- Modify: `src/pet/animator.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/layout.rs`
- Modify: `src/tui/component/watch_screen.rs`
- Modify: `src/dev_preview/strips.rs`
- Test: `src/pet/animator.rs`
- Test: `tests/watch_integration.rs`
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Write animator replay and expiry tests**

In `src/pet/animator.rs`, add tests in the existing test module:

```rust
#[test]
fn scene_moment_triggers_only_once_per_trigger_id() {
    let mut animator = SceneAnimator::new();
    let vm = WatchViewModel::fixture();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::DawnWakeWipe,
        trigger_id: crate::tui::room::SceneTriggerId::new("wake:1"),
        target_id: "watch.room.effect",
        duration_ms: 900,
        max_replay_age_ms: 3_600_000,
    };

    animator.update_scene_moments(&vm, &[moment.clone()]);
    assert!(animator.has_active_effects());
    let first_active_until = animator.active_until_ms_for_test();

    animator.update_scene_moments(&vm, &[moment]);

    assert_eq!(animator.active_until_ms_for_test(), first_active_until);
}

#[test]
fn scene_moment_expiry_controls_active_effects() {
    let mut animator = SceneAnimator::new();
    let vm = WatchViewModel::fixture();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::PropResonanceRipple,
        trigger_id: crate::tui::room::SceneTriggerId::new("prop:1"),
        target_id: "watch.room.effect",
        duration_ms: 700,
        max_replay_age_ms: 3_600_000,
    };

    animator.update_scene_moments(&vm, &[moment]);
    animator.advance_for_test(699);
    assert!(animator.has_active_effects());
    animator.advance_for_test(1);
    assert!(!animator.has_active_effects());
}
```

- [ ] **Step 2: Rename `PetAnimator` to `SceneAnimator`**

In `src/pet/animator.rs`, rename:

```rust
pub struct PetAnimator
```

to:

```rust
pub struct SceneAnimator
```

Update `impl PetAnimator` and `impl Default for PetAnimator` to `SceneAnimator`. Update imports and fields in `src/tui/app.rs` from `pet_animator` to `scene_animator`. Use the commit diff to confirm no `PetAnimator` references remain outside docs.

- [ ] **Step 3: Add scene effect keys and target areas**

Change `EffectKey`:

```rust
enum EffectKey {
    Idle,
    Hatch,
    StageUp,
    MoodFade,
    FeedPulse,
    Scene(SceneEffectKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SceneEffectKey {
    pub moment: crate::tui::room::SceneMomentKey,
    pub trigger_id: crate::tui::room::SceneTriggerId,
}
```

Change enqueue calls so each effect uses `.with_area(rect)` when a target rect is known. For existing pet effects, pass `watch.pet.effect` from the layout target set instead of relying on the process area.

Add:

```rust
pub struct SceneEffectTargets {
    pub frame: Rect,
    pub pet: Rect,
    pub room_slices: Vec<Rect>,
    pub props: std::collections::BTreeMap<&'static str, Rect>,
}
```

Derive `room_slices` by splitting `watch.room.effect` around `watch.pet.art` and `watch.pet.speech` rectangles. The split helper should return only non-empty rects.

- [ ] **Step 4: Update/apply with full-frame processing**

Change `SceneAnimator::apply` to:

```rust
pub fn apply(&mut self, targets: &SceneEffectTargets, buf: &mut Buffer, elapsed_ms: u32) {
    self.manager.process_effects(ms(elapsed_ms), buf, targets.frame);
    self.idle_ms = self.idle_ms.saturating_add(elapsed_ms);
}
```

When enqueuing room moments, create one effect per room slice with the same scene key plus slice index. When enqueuing prop moments, use the prop rect from `targets.props`.

In `src/tui/app.rs`, after layout and render:

```rust
let targets = scene_effect_targets_from_layout(&layout);
scene_animator.update(&self.vm, &targets);
scene_animator.apply(&targets, frame.buffer_mut(), elapsed_ms);
```

Make `update` derive `RoomLifeProfile` and pass `profile.scene_moments` into `update_scene_moments`.

- [ ] **Step 5: Replace smoke strip with real scene strips**

In `src/dev_preview/strips.rs`, keep `scene-strip-smoke` until all tests pass, then add real ids:

- `scene-prop-resonance-ripple`
- `scene-feed-sweep`
- `scene-dawn-wake-wipe`
- `scene-heavy-session-shimmer`

Each strip should render start/mid/end frames by creating a deterministic watch frame, applying the `SceneAnimator` for elapsed samples `0`, `duration / 2`, `duration`, and exporting the resulting frames under `strips/<id>/`.

Update `dev_preview_animation_writes_scene_strip_manifest_and_frames` to assert at least three real strips and `target_id` values for room, pet, and prop targets.

- [ ] **Step 6: Run and commit**

Run:

```bash
cargo fmt --check
cargo test pet::animator --lib
cargo test --test watch_integration
cargo test --test dev_preview dev_preview_animation_writes_scene_strip_manifest_and_frames
cargo test --test dev_preview dev_preview_all_includes_scene_strips
```

Expected: all pass.

Commit:

```bash
git add src/pet/animator.rs src/tui/app.rs src/tui/layout.rs src/tui/component/watch_screen.rs src/dev_preview tests
git commit -m "feat(tui): animate alive room scene moments"
```

## Task 8: Tune And Final Preview Gate

**Spec sections:** Testing, Success Criteria.

**Files:**
- Modify: `src/tui/room.rs`
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/dev_preview/watch.rs`
- Modify: `tests/snapshots/*.snap`

- [ ] **Step 1: Run full focused verification**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test tui::room --lib
cargo test pet::animator --lib
cargo test --test dev_preview
cargo test --test tui_render
cargo run -- dev-preview --scenario all --out target/glorp-preview
cargo run -- dev-preview --scenario animation --out target/glorp-preview-animation
```

Expected: all commands exit 0.

- [ ] **Step 2: Review generated artifacts**

Open:

```bash
open target/glorp-preview/index.html
open target/glorp-preview-animation/index.html
```

Review these exact artifacts:

- `target/glorp-preview/frames/room-starter-day-clear.room.txt`
- `target/glorp-preview/frames/room-botanical-cache-evening.room.txt`
- `target/glorp-preview/frames/room-technical-output-active.room.txt`
- `target/glorp-preview/frames/room-celestial-artifact-night.room.txt`
- `target/glorp-preview/frames/room-cozy-weekend-quiet.room.txt`
- `target/glorp-preview/frames/room-heavy-day-cozy-large.room.txt`
- `target/glorp-preview-animation/strips/scene-prop-resonance-ripple/frame-000.txt`
- `target/glorp-preview-animation/strips/scene-dawn-wake-wipe/frame-001.txt`

Acceptance checklist:

- Cropped room stills show different symbols in at least two zones.
- Botanical/cozy and technical/output do not read as the same texture.
- Pet art stays readable in every still.
- Animation strips start paused and frame stepping works.
- Room effects do not touch side panels.
- `cargo run -- watch` still launches and idles without fast ticking forever.

- [ ] **Step 3: Update snapshots deliberately**

If only expected Alive Room visuals changed, run:

```bash
cargo insta review
```

Accept only snapshots corresponding to watch room visual changes. Reject unrelated text/layout diffs.

- [ ] **Step 4: Final commit**

Run:

```bash
git status --short
```

Confirm only intended source, tests, and snapshots are modified. Commit:

```bash
git add src tests
git add tests/snapshots
git commit -m "feat(tui): ship alive room"
```

## Self-Review Checklist For Plan Executor

- [ ] Preview strips land before any implementation relies on animation review.
- [ ] `RoomLifeProfile` is pure and contains no render-clock-only motion state.
- [ ] Prop effect targets come from rendered prop placements, not duplicate preview math.
- [ ] Static catalog-backed `TargetPath` ids are used; no owned target-path refactor is needed.
- [ ] Preview layout targets still expose `x`, `y`, `width`, and `height` at top level for existing tests.
- [ ] Room effects are split around pet/speech targets before being enqueued.
- [ ] Resting room motion does not call `SceneAnimator::enqueue` and does not keep fast tick active.
- [ ] Low-color tests compare symbol changes, not RGB-only differences.
- [ ] The generated preview bundle is the final visual arbiter before merging.
