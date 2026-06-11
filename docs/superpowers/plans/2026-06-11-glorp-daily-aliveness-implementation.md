# Glorp Daily Aliveness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the glorp watch scene look meaningfully different through the day and across kinds-of-day — finishing the Alive Room resting-state intent (room phase composition, pet posture/eyes), correcting two pet.jsx gaps (breath cadence, Ecstatic mood), adding sleep/idle micro-life, and adding one net-new channel: the pet's face reacting to the kind of work flowing.

**Architecture:** All changes are to *render consumers* of already-derived state (`RoomLifeProfile`, `DayContext`, `PetLifeProfile`, `PetPerformance`). No new poll-time derivation, no new contract, no pipeline change. New per-frame presentation inputs (`soft_eyes`, `work_accent`) ride on the existing `AnimationFrame`. Every behavior keys off a real attribute, never the clock alone (locked texture-vs-personality boundary).

**Tech Stack:** Rust, ratatui, `time` crate, the `glorp dev-preview` Preview Lab for deterministic visual review. TDD throughout: failing test → minimal impl → green → commit.

**Spec:** `docs/superpowers/specs/2026-06-11-glorp-daily-aliveness-design.md`

---

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `src/tui/room.rs` | Modify | A: `room_glyphs_for` gains `day_phase`; add `phase_warmth_tint`, `phase_density_scale` |
| `src/tui/panels/pet.rs` | Modify | A: pass `day_phase` at call site. B: `performance_lightness_multiplier`, `performance_posture_offset`, set `soft_eyes`. E: derive `work_accent` is in watch.rs, but pet.rs render path consumes the frame |
| `src/pet/render.rs` | Modify | C2: `Ecstatic` expression arm + blink-block. B: `soft_eyes` in `expression_for`. E: `WorkAccent` enum + accent application in `expression_for`; `AnimationFrame` gains `soft_eyes` + `work_accent` |
| `src/pet/animator.rs` | Modify | C1: re-align `species_breath_rhythm_decis`. D: sleep-depth shallower inhale over onset; idle laziness in wander/twinkle |
| `src/game/metabolism.rs` | Modify | C2: `Ecstatic` enum variant, round-trip, `mood_for` threshold |
| `src/pet/narration.rs`, `src/pet/speech.rs` | Modify | C2: add `Ecstatic` arms to exhaustive `match mood` |
| `src/commands/watch.rs`, `src/tui/layout.rs`, `src/dev_preview/pets.rs` | Modify | Update `AnimationFrame` construction sites; live producers compute `soft_eyes`/`work_accent` |
| `src/dev_preview/watch.rs`, `src/dev_preview/scenarios.rs` | Modify | Preview fixtures for phase × kind-of-day × work_weather |
| `tests/dev_preview.rs` | Modify | Assert new preview scenarios |

**Build/lint/test commands (used throughout):**
- Test one: `cargo test --lib <module>::<test_name>`
- Test file: `cargo test --test dev_preview`
- Full: `cargo test`
- Lint gate: `cargo clippy --all-targets --all-features -- -D warnings`
- Format: `cargo fmt`

---

## Phase A — Room phase composition + warmth

Make the room interior vary by `DayPhase`: night sparser + dim, dusk warmer + lower contrast, dawn cooler, day full. Mirrors the existing sky-phase pattern (`pet.rs:303` `phase_count_scale`, `pet.rs:337` `climate_tint`). Low-churn: changes happen inside `room_glyphs_for`.

### Task A1: Phase warmth + density helpers (pure functions, TDD)

**Files:**
- Modify: `src/tui/room.rs`
- Test: inline `#[cfg(test)]` in `src/tui/room.rs`

- [ ] **Step 1: Write failing tests for the two helpers**

Add to the `#[cfg(test)] mod tests` block in `src/tui/room.rs`:

```rust
#[test]
fn phase_density_scale_is_sparsest_at_night_fullest_at_day() {
    assert_eq!(phase_density_scale(DayPhase::Day), 1.0);
    assert!(phase_density_scale(DayPhase::Night) < phase_density_scale(DayPhase::Dusk));
    assert!(phase_density_scale(DayPhase::Dawn) < phase_density_scale(DayPhase::Day));
    assert!(phase_density_scale(DayPhase::Night) < phase_density_scale(DayPhase::Dawn));
}

#[test]
fn phase_warmth_tint_warms_at_dusk_and_cools_at_dawn() {
    let base = Color::Rgb(120, 120, 120);
    let Color::Rgb(dawn_r, _, dawn_b) = phase_warmth_tint(base, DayPhase::Dawn) else {
        panic!("expected rgb");
    };
    let Color::Rgb(dusk_r, _, dusk_b) = phase_warmth_tint(base, DayPhase::Dusk) else {
        panic!("expected rgb");
    };
    // Dusk is warmer (more red, less blue) than dawn.
    assert!(dusk_r > dawn_r, "dusk should be redder than dawn");
    assert!(dusk_b < dawn_b, "dusk should be less blue than dawn");
    // Day is identity.
    assert_eq!(phase_warmth_tint(base, DayPhase::Day), base);
}
```

- [ ] **Step 2: Run tests, confirm they fail**

Run: `cargo test --lib tui::room::tests::phase_density_scale_is_sparsest_at_night_fullest_at_day`
Expected: FAIL — `cannot find function phase_density_scale`.

- [ ] **Step 3: Implement the helpers**

Add near the other room helpers in `src/tui/room.rs` (e.g. just above `motion_budget` at line 452):

```rust
/// Interior ambient-glyph density scale by phase. Night sparse, day full.
/// Mirrors the sky's `phase_count_scale`. Texture only.
fn phase_density_scale(phase: DayPhase) -> f64 {
    match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.7,
        DayPhase::Dusk => 0.85,
        DayPhase::Night => 0.5,
    }
}

/// Warmth/contrast bias applied to a room glyph color by phase: dawn cooler,
/// day neutral, dusk warmer, night dim. Texture only — never personality.
fn phase_warmth_tint(color: Color, phase: DayPhase) -> Color {
    let Color::Rgb(r, g, b) = color else {
        return color;
    };
    match phase {
        DayPhase::Day => color,
        DayPhase::Dawn => Color::Rgb(r.saturating_sub(6), g, b.saturating_add(8)),
        DayPhase::Dusk => {
            Color::Rgb(r.saturating_add(12), g.saturating_add(2), b.saturating_sub(8))
        }
        DayPhase::Night => {
            Color::Rgb(r.saturating_sub(14), g.saturating_sub(10), b.saturating_sub(2))
        }
    }
}
```

Ensure `DayPhase` is imported (already is, room.rs:8) and `Color` is in scope (used by `RoomGlyph.style`).

- [ ] **Step 4: Run tests, confirm pass**

Run: `cargo test --lib tui::room::tests::phase_density_scale_is_sparsest_at_night_fullest_at_day tui::room::tests::phase_warmth_tint_warms_at_dusk_and_cools_at_dawn`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add src/tui/room.rs
git commit -m "feat(tui): add room phase warmth + density helpers"
```

### Task A2: Apply phase inside room_glyphs_for (TDD)

**Files:**
- Modify: `src/tui/room.rs` (`room_glyphs_for` signature + body)
- Modify: `src/tui/panels/pet.rs:885` (call site)
- Test: inline in `src/tui/room.rs`

- [ ] **Step 1: Write failing tests for phase-aware room glyphs**

Add to `src/tui/room.rs` tests (reuse the existing fixture style from `room_glyphs_are_deterministic_for_identical_inputs`):

```rust
fn phase_test_profile() -> RoomLifeProfile {
    RoomLifeProfile {
        biome: RoomBiome {
            primary: RoomBiomeTag::Botanical,
            secondary: Some(RoomBiomeTag::Cozy),
        },
        room_weather: RoomWeatherLayer::Clear,
        resonant_emitter: None,
        pet_performance: PetPerformance::RestedAwake,
        scene_moments: Vec::new(),
        identity_prop_ids: Vec::new(),
    }
}

#[test]
fn room_glyphs_are_sparser_at_night_than_day() {
    let profile = phase_test_profile();
    let area = Rect::new(0, 0, 120, 32);
    let now = datetime!(2026-06-11 10:00 UTC);
    let day = room_glyphs_for(&profile, area, &[], now, ColorCapability::Truecolor, DayPhase::Day);
    let night =
        room_glyphs_for(&profile, area, &[], now, ColorCapability::Truecolor, DayPhase::Night);
    assert!(
        night.len() < day.len(),
        "night ({}) should have fewer ambient glyphs than day ({})",
        night.len(),
        day.len()
    );
}

#[test]
fn room_glyphs_warm_at_dusk_in_color_mode() {
    let profile = phase_test_profile();
    let area = Rect::new(0, 0, 120, 32);
    let now = datetime!(2026-06-11 10:00 UTC);
    let day = room_glyphs_for(&profile, area, &[], now, ColorCapability::Truecolor, DayPhase::Day);
    let dusk =
        room_glyphs_for(&profile, area, &[], now, ColorCapability::Truecolor, DayPhase::Dusk);
    // At least one co-located glyph should be warmer (redder) at dusk.
    let warmer = day.iter().any(|d| {
        dusk.iter().any(|k| {
            k.row == d.row
                && k.col == d.col
                && matches!((k.style.fg, d.style.fg),
                    (Some(Color::Rgb(kr, _, _)), Some(Color::Rgb(dr, _, _))) if kr > dr)
        })
    });
    assert!(warmer, "dusk should warm at least one room glyph vs day");
}

#[test]
fn room_glyphs_still_emit_in_flat_mode_at_night() {
    let profile = phase_test_profile();
    let area = Rect::new(0, 0, 120, 32);
    let now = datetime!(2026-06-11 10:00 UTC);
    let glyphs =
        room_glyphs_for(&profile, area, &[], now, ColorCapability::Flat, DayPhase::Night);
    assert!(!glyphs.is_empty(), "flat night should still emit room glyphs");
    let faint = tokenpet_palette().faint.rgb;
    for g in &glyphs {
        assert_eq!(g.style.fg, Some(faint), "flat mode keeps the faint color (no warmth)");
    }
}
```

- [ ] **Step 2: Run tests, confirm they fail to compile**

Run: `cargo test --lib tui::room::tests::room_glyphs_are_sparser_at_night_than_day`
Expected: FAIL — `room_glyphs_for` takes 5 args, not 6.

- [ ] **Step 3: Thread `day_phase` through `room_glyphs_for`**

Change the signature and tail of `room_glyphs_for` in `src/tui/room.rs:425`:

```rust
pub fn room_glyphs_for(
    profile: &RoomLifeProfile,
    area: Rect,
    exclusions: &[Rect],
    now: OffsetDateTime,
    color_capability: ColorCapability,
    day_phase: DayPhase,
) -> Vec<RoomGlyph> {
    let mut cells: std::collections::HashMap<(u16, u16), RoomGlyph> =
        std::collections::HashMap::new();
    for glyph in biome_glyphs(profile, area, now, color_capability) {
        cells.insert((glyph.row, glyph.col), glyph);
    }
    for glyph in weather_glyphs(profile, area, now, color_capability) {
        cells.insert((glyph.row, glyph.col), glyph);
    }
    for glyph in emitter_glyphs(profile, area, now, color_capability) {
        cells.insert((glyph.row, glyph.col), glyph);
    }
    let mut glyphs: Vec<RoomGlyph> = cells.into_values().collect();
    glyphs.sort_by_key(|g| (g.row, g.col));
    let budget = (motion_budget(area) as f64 * phase_density_scale(day_phase)).round() as usize;
    let flat = matches!(color_capability, ColorCapability::Flat);
    glyphs
        .into_iter()
        .filter(|glyph| !rects_contain(exclusions, glyph.col, glyph.row))
        .take(budget)
        .map(|mut glyph| {
            if !flat {
                if let Some(fg) = glyph.style.fg {
                    glyph.style = glyph.style.fg(phase_warmth_tint(fg, day_phase));
                }
            }
            glyph
        })
        .collect()
}
```

- [ ] **Step 4: Update the call site**

In `src/tui/panels/pet.rs` at the `room_glyphs_for` call (around line 885), append the phase argument:

```rust
let room_glyphs = crate::tui::room::room_glyphs_for(
    &room_profile,
    scene.habitat,
    &ambient_exclusions,
    now,
    ctx.color_capability,
    vm.day_context.day_phase,
);
```

- [ ] **Step 5: Fix every other call site, including existing tests**

Run: `grep -rn "room_glyphs_for(" src/ tests/`
Add the `day_phase` argument to **every** caller — the existing room.rs tests (`room_glyphs_are_deterministic_for_identical_inputs`, `room_glyphs_use_symbol_families_in_flat_mode`) pass `DayPhase::Day`; preview/room fixtures pass the scenario's phase; production callers pass `vm.day_context.day_phase`.

- [ ] **Step 6: Run tests + clippy**

Run: `cargo test --lib tui::room:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 7: Commit**

```bash
git add src/tui/room.rs src/tui/panels/pet.rs
git commit -m "feat(tui): vary room interior density and warmth by day phase"
```

### Task A3: Preview fixtures for room phase (dawn/dusk/night)

**Files:**
- Modify: `src/dev_preview/watch.rs` (add fixtures), `src/dev_preview/scenarios.rs` (inputs)
- Test: `tests/dev_preview.rs`

- [ ] **Step 1: Write a failing integration assertion**

In `tests/dev_preview.rs`, extend the day-context scenario list / add a test asserting new ids exist. Follow the existing `dev_preview_watch_includes_day_context_inputs` pattern (around line 335):

```rust
#[test]
fn dev_preview_includes_room_phase_scenarios() {
    let run = PreviewRun::new();
    run.run_success("watch");
    let manifest = run.manifest();
    for id in ["watch-daycontext-dawn-fresh", "watch-daycontext-dusk-heavy", "watch-daycontext-night-quiet"] {
        let s = scenario(&manifest, id);
        assert!(s["inputs"]["day_context"]["day_phase"].is_string(), "{id} needs a day_phase");
    }
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --test dev_preview dev_preview_includes_room_phase_scenarios`
Expected: FAIL — `missing scenario watch-daycontext-dawn-fresh`.

- [ ] **Step 3: Add the fixtures**

In `src/dev_preview/watch.rs`, add three `fn <name>_day_context(now) -> DayContext` builders (mirror `light_day_morning_day_context` at line 1273 and `heavy_day_evening_day_context` at 1259), e.g.:

```rust
fn dawn_fresh_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dawn,
        phase_started_at_utc: now - Duration::minutes(40),
        phase_ends_at_utc: now + Duration::minutes(80),
        today_ratio: 0.05,
        tiredness: 0.0,
        mature: true,
        local_day_started_utc: now - Duration::hours(1),
        local_day_rollover_utc: now + Duration::hours(23),
        ..DayContext::default()
    }
}

fn dusk_heavy_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Dusk,
        phase_started_at_utc: now - Duration::hours(1),
        phase_ends_at_utc: now + Duration::hours(2),
        today_ratio: 1.6,
        tiredness: 0.8,
        mature: true,
        local_day_started_utc: now - Duration::hours(12),
        local_day_rollover_utc: now + Duration::hours(12),
        ..DayContext::default()
    }
}

fn night_quiet_day_context(now: OffsetDateTime) -> DayContext {
    DayContext {
        day_phase: DayPhase::Night,
        phase_started_at_utc: now - Duration::hours(2),
        phase_ends_at_utc: now + Duration::hours(6),
        today_ratio: 0.1,
        tiredness: 0.3,
        mature: true,
        local_day_started_utc: now - Duration::hours(16),
        local_day_rollover_utc: now + Duration::hours(8),
        ..DayContext::default()
    }
}
```

Register them in `day_context_frame_fixtures` (line 340) as three new `DayContextFrameFixture` entries with ids `watch-daycontext-dawn-fresh`, `watch-daycontext-dusk-heavy`, `watch-daycontext-night-quiet`, an appropriate `title`, the `cooling_life_profile()`/`idle_life_profile()` life fixture, and `hold_eyes_closed: false`.

- [ ] **Step 4: Wire the manifest inputs**

In `src/dev_preview/scenarios.rs`, add the three ids to the `day_context_inputs_for_frame` match (line 646 area), returning the matching `(day_phase, phase_started, phase_ends, asleep=false, sleep_onset=None, wake_resume=None, blend, life_profile, extras)` tuple per fixture.

- [ ] **Step 5: Run, confirm pass + visually review**

Run: `cargo test --test dev_preview dev_preview_includes_room_phase_scenarios`
Expected: PASS.
Then: `cargo run -- dev-preview --scenario watch --out target/glorp-preview && open target/glorp-preview/index.html` — confirm dawn/dusk/night rooms read distinctly (warm dusk, dim night).

- [ ] **Step 6: Commit**

```bash
git add src/dev_preview/watch.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "test(preview): add room phase fixtures (dawn/dusk/night)"
```

---

## Phase B — Pet performance posture + eyes

Extend `PetPerformance` rendering past the one-cell cue: a resting brightness baseline (composed *under* the activity lift), a ≤1-row posture offset, and soft eyes for tired/cozy. Keys off `PetPerformance` only — no clock.

### Task B1: Performance brightness baseline (TDD)

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Test: inline in `src/tui/panels/pet.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn performance_lightness_baseline_dims_tired_and_asleep_below_rested() {
    let rested = performance_lightness_multiplier(crate::tui::room::PetPerformance::RestedAwake);
    let tired = performance_lightness_multiplier(crate::tui::room::PetPerformance::TiredAwake);
    let asleep = performance_lightness_multiplier(crate::tui::room::PetPerformance::AsleepDreaming);
    assert_eq!(rested, 1.0, "rested is the neutral baseline");
    assert!(tired < rested, "tired sits below rested");
    assert!(asleep < tired, "asleep is the dimmest");
    assert!(asleep > 0.5, "never fully dark");
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib tui::panels::pet::tests::performance_lightness_baseline_dims_tired_and_asleep_below_rested`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement the multiplier**

Add near `low_energy_lightness_multiplier` usage in `src/tui/panels/pet.rs`:

```rust
/// Resting brightness baseline by performance state, composed UNDER the
/// activity lift (a tired pet still visibly brightens when work arrives, it
/// just settles back lower). 1.0 = neutral. Bounded so the pet is never dark.
fn performance_lightness_multiplier(performance: crate::tui::room::PetPerformance) -> f32 {
    use crate::tui::room::PetPerformance::*;
    match performance {
        RestedAwake | CatchUpWake | SourceBurstPerk => 1.0,
        TiredAwake => 0.88,
        HeavyDayCozy => 0.82,
        AsleepDreaming => 0.7,
    }
}
```

- [ ] **Step 4: Compose it under activity lift in `render_pet_inside`**

In `src/tui/panels/pet.rs` `render_pet_inside` (around line 1116 where `low_energy_lightness_multiplier` is applied), fold the performance baseline into the same darken step. The `room_profile.pet_performance` must be in scope here — thread it in (the parent at line 1003 already has `room_profile`; pass `room_profile.pet_performance` into `render_pet_inside` or read it before the call). Apply as:

```rust
let energy_m = low_energy_lightness_multiplier(vm.energy);
let perf_m = performance_lightness_multiplier(pet_performance);
let droop = darken_pet_styles(&base, energy_m * perf_m);
```

(`pet_performance` is the new parameter passed into `render_pet_inside`.) Activity lift still applies afterward, unchanged.

- [ ] **Step 5: Run tests + clippy**

Run: `cargo test --lib tui::panels::pet:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 6: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "feat(tui): resting brightness baseline per pet performance"
```

### Task B2: Posture offset (≤1 row) for settled states (TDD)

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Test: inline

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn posture_offset_settles_tired_cozy_asleep_one_row() {
    use crate::tui::room::PetPerformance::*;
    assert_eq!(performance_posture_offset(RestedAwake), 0);
    assert_eq!(performance_posture_offset(SourceBurstPerk), 0);
    assert_eq!(performance_posture_offset(TiredAwake), 1);
    assert_eq!(performance_posture_offset(HeavyDayCozy), 1);
    assert_eq!(performance_posture_offset(AsleepDreaming), 1);
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib tui::panels::pet::tests::posture_offset_settles_tired_cozy_asleep_one_row`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement + apply**

```rust
/// Resting vertical offset (rows) by performance state. Settled states sit
/// one row lower; alert/rested stay put. Capped at 1 to preserve the quiet
/// halo around the pet.
fn performance_posture_offset(performance: crate::tui::room::PetPerformance) -> u16 {
    use crate::tui::room::PetPerformance::*;
    match performance {
        TiredAwake | HeavyDayCozy | AsleepDreaming => 1,
        RestedAwake | CatchUpWake | SourceBurstPerk => 0,
    }
}
```

Apply where the pet art rect's `y` is set in `render_pet_inside` (the `scene.pet_art` rect), clamping so the offset never pushes the art below the habitat:

```rust
let posture = performance_posture_offset(pet_performance);
let pet_rect = {
    let mut r = scene.pet_art;
    let max_y = scene.habitat.y + scene.habitat.height.saturating_sub(r.height);
    r.y = (r.y + posture).min(max_y);
    r
};
// use pet_rect in place of scene.pet_art for render_pet_lines_sparse
```

- [ ] **Step 4: Run tests + clippy; visually confirm no clipping**

Run: `cargo test --lib tui::panels::pet:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "feat(tui): settle pet one row for tired/cozy/asleep performance"
```

### Task B3: Soft eyes on AnimationFrame (TDD)

This adds the `soft_eyes` field to `AnimationFrame` and applies it in `expression_for`. (The `work_accent` field is added in Phase E — to avoid touching every construction site twice, **add both fields here** and leave `work_accent` inert until Phase E.)

**Files:**
- Modify: `src/pet/render.rs` (`AnimationFrame`, `expression_for`)
- Modify: all `AnimationFrame { ... }` sites (8 total)
- Test: inline in `src/pet/render.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn soft_eyes_relax_a_positive_mood_without_changing_mouth() {
    let pet = generate_pet("soft-eyes-seed");
    // soft_eyes + work_accent are added in Step 3; this won't compile until then.
    let normal = AnimationFrame {
        tick: 1,
        blink_suppression_ticks: 0,
        hold_eyes_closed: false,
        blink_slowdown: 0,
        soft_eyes: false,
        work_accent: WorkAccent::None,
    };
    let soft = AnimationFrame { soft_eyes: true, ..normal };
    let a = render_pet(&pet, Stage::S3, Mood::Content, normal).lines.join("\n");
    let b = render_pet(&pet, Stage::S3, Mood::Content, soft).lines.join("\n");
    assert_ne!(a, b, "soft eyes should change the rendered face");
}
```

- [ ] **Step 2: Run, confirm failure to compile**

Run: `cargo test --lib pet::render::tests::soft_eyes_relax_a_positive_mood_without_changing_mouth`
Expected: FAIL — `AnimationFrame` has no field `soft_eyes`.

- [ ] **Step 3: Add the fields + `WorkAccent` enum (used in E) + `Default`**

In `src/pet/render.rs`, extend `AnimationFrame` (line 7) and add the enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WorkAccent {
    #[default]
    None,
    /// Output-heavy bursts: brighter, sharper eyes.
    Alert,
    /// Reasoning-heavy: narrowed, focused eyes.
    Focused,
    /// Cache-heavy: softer, dreamier eyes.
    Dreamy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationFrame {
    pub tick: u64,
    pub blink_suppression_ticks: u8,
    pub hold_eyes_closed: bool,
    pub blink_slowdown: u8,
    /// Relax the eyes for tired/cozy performance (B). Inert for closed/blink.
    pub soft_eyes: bool,
    /// Subtle work-type expression accent (E). Applied only to positive moods.
    pub work_accent: WorkAccent,
}
```

(Use the `WorkAccent::None` variant instead of `Option<WorkAccent>` so the field is `Copy`+`Default` with no `Option` noise — matching the Step 1 test.)

- [ ] **Step 4: Apply `soft_eyes` in `expression_for`**

Thread `frame` (or just the two flags) into `expression_for`. Simplest: change its signature to take `frame: AnimationFrame` and apply softening for positive moods when not blinking:

```rust
fn expression_for(pet: &GeneratedPet, mood: Mood, blinking: bool, frame: AnimationFrame) -> Expression {
    if blinking {
        return Expression {
            eyes: closed_blink_eyes(pet.species).to_string(),
            mouth: pet.traits.mouth.clone(),
        };
    }
    let mut expr = match mood {
        Mood::Happy => Expression { eyes: "^.^".to_string(), mouth: "\u{03c9}".to_string() },
        Mood::Content => Expression { eyes: pet.traits.eyes.clone(), mouth: pet.traits.mouth.clone() },
        Mood::Hungry => Expression { eyes: "u.u".to_string(), mouth: "o".to_string() },
        Mood::Sad => Expression { eyes: "T.T".to_string(), mouth: "\u{fe35}".to_string() },
        Mood::Sleepy => Expression { eyes: "-.-".to_string(), mouth: "-".to_string() },
        Mood::Wilted => Expression { eyes: ",_,".to_string(), mouth: "_".to_string() },
        // Mood::Ecstatic arm added in Phase C2.
    };
    if frame.soft_eyes && matches!(mood, Mood::Content | Mood::Happy) {
        expr.eyes = "\u{02d8}.\u{02d8}".to_string(); // ˘.˘ relaxed, heavy-lidded
    }
    expr
}
```

Update its caller at `render.rs:81`: `let expression = expression_for(pet, mood, blinking, frame);`

- [ ] **Step 5: Update all 8 `AnimationFrame` construction sites**

Add `soft_eyes` + `work_accent` to each literal. Run `grep -rn "AnimationFrame {" src/` and update:
- `src/commands/watch.rs:81` and `:436` (live producers): `soft_eyes: <from performance>, work_accent: WorkAccent::None,` (E fills work_accent later; B's soft_eyes computed from `room_profile.pet_performance` — `matches!(perf, TiredAwake | HeavyDayCozy)`).
- `src/tui/layout.rs:471`: same as watch producers.
- `src/tui/panels/pet.rs:1651`, `src/pet/render.rs:632/653/689`, `src/dev_preview/pets.rs:69` (tests/preview): `soft_eyes: false, work_accent: WorkAccent::None,` (or `..AnimationFrame::default()` now that Default is derived).

- [ ] **Step 6: Run full build + tests + clippy**

Run: `cargo test --lib pet::render:: && cargo build && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 7: Wire `soft_eyes` from performance at the live producers**

In `src/commands/watch.rs` (the two producers) set `soft_eyes` from the same `pet_performance` the room derives, e.g.:

```rust
let soft_eyes = matches!(
    pet_performance,
    crate::tui::room::PetPerformance::TiredAwake | crate::tui::room::PetPerformance::HeavyDayCozy
);
```

(Compute `pet_performance` via `derive_room_life_profile` or the existing `pet_performance_for` and reuse it.)

- [ ] **Step 8: Commit**

```bash
git add src/pet/render.rs src/commands/watch.rs src/tui/layout.rs src/tui/panels/pet.rs src/dev_preview/pets.rs
git commit -m "feat(pet): soft eyes for tired/cozy performance via AnimationFrame"
```

---

## Phase C — Breath cadence re-align + Ecstatic mood

### Task C1: Re-align breath cadence to pet.jsx (TDD)

**Files:**
- Modify: `src/pet/animator.rs` (`species_breath_rhythm_decis`, two existing tests)
- Test: inline in `src/pet/animator.rs`

- [ ] **Step 1: Write the failing ordering test**

```rust
#[test]
fn breath_periods_match_pet_jsx_ordering() {
    // pet.jsx SPECIES_ANIM breathPeriod: glitch 9 < ghost 11 < blob 13 < fuzz 16 < mech 17 < crystal 19.
    let p = |s| species_breath_rhythm_decis(Some(s)).0;
    assert!(p(Species::Glitch) < p(Species::Ghost));
    assert!(p(Species::Ghost) < p(Species::Blob));
    assert!(p(Species::Blob) < p(Species::Fuzz));
    assert!(p(Species::Fuzz) <= p(Species::Mech));
    assert!(p(Species::Mech) < p(Species::Crystal));
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib pet::animator::tests::breath_periods_match_pet_jsx_ordering`
Expected: FAIL — current values have ghost(45) > blob(50)? No: ghost 45 < blob 50 but fuzz 40 < ghost 45 fails ordering (blob 50 > fuzz 40), so assertion `p(Blob) < p(Fuzz)` fails.

- [ ] **Step 3: Re-align the table**

Replace `species_breath_rhythm_decis` body (`animator.rs:478`) with pet.jsx proportions at ~200 ms/fast-tick (decis = breathPeriod × 2, inhale = breathHold × 2):

```rust
fn species_breath_rhythm_decis(species: Option<Species>) -> (i64, i64) {
    match species {
        Some(Species::Glitch) => (18, 4),
        Some(Species::Ghost) => (22, 6),
        Some(Species::Blob) => (26, 10),
        Some(Species::Fuzz) => (32, 8),
        Some(Species::Mech) => (34, 8),
        Some(Species::Crystal) => (38, 12),
        None => (32, 8),
    }
}
```

- [ ] **Step 4: Run the new test (pass) and the existing breath tests (expect 2 failures)**

Run: `cargo test --lib pet::animator::`
Expected: `breath_periods_match_pet_jsx_ordering` PASS; `breath_rhythm_differs_per_species` PASS; but `tired_breath_period_scale_at_full_eighths_equals_tired_breath_max_scale` and `sleep_breath_is_slower_and_continuous_at_onset` FAIL — their hard-coded edge counts / timestamps assumed the old periods.

- [ ] **Step 5: Update `tired_breath_period_scale_*` expected counts**

In that test (animator.rs:1320), the rising-edge counts change with Crystal's new 38ds period. Replace the two `assert_eq!` literals with the values the failure output prints (the ratio `awake/tired` must still equal `TIRED_BREATH_MAX_SCALE`). The structure stays; only `assert_eq!(awake, 30)` / `assert_eq!(tired, 20)` get the new numbers from the run.

- [ ] **Step 6: Update `sleep_breath_is_slower_and_continuous_at_onset`**

Fuzz asleep period is now `32 * SLEEP_BREATH_PERIOD_SCALE`. Re-derive the sample points: `at_onset` (phase 0, in inhale) still `1`; recompute the mid (rest) and next-cycle timestamps from the new period and update the `+5s`/`+12s` samples and comment to the new cycle boundary (next inhale at one full asleep period after onset). Use values from the failure output.

- [ ] **Step 7: Run all animator tests, confirm green**

Run: `cargo test --lib pet::animator::`
Expected: PASS.

- [ ] **Step 8: Visually confirm breathing in preview**

Run: `cargo run -- dev-preview --scenario pets --out target/glorp-preview` (and the animation strips) — confirm breathing reads natural (glitch twitchy, crystal slow, ghost/blob livelier than before).

- [ ] **Step 9: Commit**

```bash
git add src/pet/animator.rs
git commit -m "fix(pet): re-align species breath cadence to pet.jsx proportions"
```

### Task C2: Add the Ecstatic mood (TDD)

**Files:**
- Modify: `src/game/metabolism.rs` (enum, round-trip, `mood_for`)
- Modify: `src/pet/render.rs` (`expression_for`, `should_blink`)
- Modify: `src/pet/narration.rs`, `src/pet/speech.rs` (exhaustive matches)
- Test: inline in `src/game/metabolism.rs` and `src/pet/render.rs`

- [ ] **Step 1: Write failing tests**

In `src/game/metabolism.rs` tests:

```rust
#[test]
fn peak_vitals_are_ecstatic_and_round_trip() {
    let peak = Vitals { fed: 95.0, happiness: 95.0, energy: 80.0 };
    assert_eq!(mood_for_vitals(peak), Mood::Ecstatic);
    // Just-happy stays Happy.
    let happy = Vitals { fed: 80.0, happiness: 80.0, energy: 60.0 };
    assert_eq!(mood_for_vitals(happy), Mood::Happy);
    assert_eq!("ecstatic".parse::<Mood>().unwrap(), Mood::Ecstatic);
    assert_eq!(Mood::Ecstatic.as_str(), "ecstatic");
}
```

In `src/pet/render.rs` tests:

```rust
#[test]
fn ecstatic_renders_the_star_eyes_and_blocks_blink() {
    let pet = generate_pet("ecstatic-seed");
    let frame = AnimationFrame { tick: 1, ..AnimationFrame::default() };
    let art = render_pet(&pet, Stage::S4, Mood::Ecstatic, frame).lines.join("\n");
    assert!(art.contains("*o*"), "ecstatic uses star eyes, got:\n{art}");
}
```

- [ ] **Step 2: Run, confirm failures (compile errors)**

Run: `cargo test --lib metabolism::tests::peak_vitals_are_ecstatic_and_round_trip`
Expected: FAIL — `Mood::Ecstatic` does not exist.

- [ ] **Step 3: Add the enum variant + round-trip**

In `src/game/metabolism.rs` add `Ecstatic` to the enum (after `Happy`), to `as_str` (`Mood::Ecstatic => "ecstatic"`), and to `from_str` (`"ecstatic" => Ok(Mood::Ecstatic)`). The `#[serde(rename_all = "lowercase")]` handles serialization automatically.

- [ ] **Step 4: Add the `mood_for` threshold**

In `mood_for` (metabolism.rs:229), insert the Ecstatic branch ABOVE the Happy branch:

```rust
} else if vitals.fed >= 90.0 && vitals.happiness >= 90.0 && vitals.energy >= 70.0 {
    Mood::Ecstatic
} else if vitals.fed >= 75.0 && vitals.happiness >= 75.0 && vitals.energy >= 55.0 {
    Mood::Happy
} else {
    Mood::Content
}
```

- [ ] **Step 5: Add the expression arm + blink-block**

In `src/pet/render.rs` `expression_for`, add after the `Happy` arm:

```rust
Mood::Ecstatic => Expression {
    eyes: "*o*".to_string(),
    mouth: "\u{25bd}".to_string(), // ▽
},
```

In `should_blink` (render.rs:257), add `Ecstatic` to the blink-block set (pet.jsx BLINK_BLOCKED_MOODS includes `ecstatic`):

```rust
if matches!(mood, Mood::Sad | Mood::Sleepy | Mood::Wilted | Mood::Ecstatic) {
    return false;
}
```

- [ ] **Step 6: Fix all other exhaustive `match mood` arms**

Run: `grep -rn "Mood::Happy" src/` and `grep -rn "match.*mood" src/` to find every exhaustive match. Add an `Ecstatic` arm to each:
- `src/pet/narration.rs` (e.g. line 111): give Ecstatic upbeat lines, e.g. `Mood::Ecstatic => &["{name} is beaming", "{name} glows"],`
- `src/pet/speech.rs` (e.g. line 184): `Mood::Ecstatic => &["!!!", "so good", "best day"],`
- Any others the compiler flags. Let the compiler drive: `cargo build` reports each non-exhaustive match.

- [ ] **Step 7: Run tests + build + clippy**

Run: `cargo test --lib metabolism:: pet::render:: && cargo build && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 8: Commit**

```bash
git add src/game/metabolism.rs src/pet/render.rs src/pet/narration.rs src/pet/speech.rs
git commit -m "feat(pet): add Ecstatic mood with star-eyed face at peak vitals"
```

---

## Phase E — Work-type expression accent

The pet's face reacts to `work_weather` while work is live. Subtle, modifier-not-override, mood-gated, live-gated. Uses the `work_accent` field added on `AnimationFrame` in B3.

### Task E1: Apply work accent in expression_for (TDD)

**Files:**
- Modify: `src/pet/render.rs` (`expression_for`)
- Test: inline in `src/pet/render.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn work_accent_sharpens_only_positive_moods() {
    let pet = generate_pet("accent-seed");
    let base = AnimationFrame { tick: 2, ..AnimationFrame::default() };
    let alert = AnimationFrame { work_accent: WorkAccent::Alert, ..base };
    // Positive mood: accent changes the face.
    let happy_plain = render_pet(&pet, Stage::S3, Mood::Happy, base).lines.join("\n");
    let happy_alert = render_pet(&pet, Stage::S3, Mood::Happy, alert).lines.join("\n");
    assert_ne!(happy_plain, happy_alert, "alert accent should change a happy face");
    // Negative mood: accent is ignored (honest face).
    let sad_plain = render_pet(&pet, Stage::S3, Mood::Sad, base).lines.join("\n");
    let sad_alert = render_pet(&pet, Stage::S3, Mood::Sad, alert).lines.join("\n");
    assert_eq!(sad_plain, sad_alert, "a sad pet keeps its honest face");
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib pet::render::tests::work_accent_sharpens_only_positive_moods`
Expected: FAIL — accent has no effect yet (both equal).

- [ ] **Step 3: Apply the accent in `expression_for`**

After the `soft_eyes` block (and after the mood match), in `expression_for`:

```rust
if !matches!(mood, Mood::Sad | Mood::Hungry | Mood::Sleepy | Mood::Wilted) {
    match frame.work_accent {
        WorkAccent::None => {}
        WorkAccent::Alert => expr.eyes = "^o^".to_string(),
        WorkAccent::Focused => expr.eyes = ">.<".to_string(),
        WorkAccent::Dreamy => expr.eyes = "u.u".to_string(),
    }
}
```

(Soft eyes and work accent are mutually exclusive in practice — soft eyes fires for tired/cozy performance, which gates `work_accent` off via the live-work gate in E2. If both are set, accent wins as the more transient signal; the test only sets one.)

- [ ] **Step 4: Run test + clippy**

Run: `cargo test --lib pet::render:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add src/pet/render.rs
git commit -m "feat(pet): work-type expression accent for positive moods"
```

### Task E2: Derive work_accent at the live producers (TDD)

**Files:**
- Modify: `src/commands/watch.rs` (the two `AnimationFrame` producers), `src/tui/layout.rs`
- Add: a small pure mapper (in `src/pet/render.rs` or `src/tui/life.rs`)
- Test: inline where the mapper lives

- [ ] **Step 1: Write failing test for the mapper**

In `src/pet/render.rs` (or wherever the mapper lands):

```rust
#[test]
fn work_accent_from_weather_gates_on_live_work() {
    use crate::tui::life::WorkWeather;
    // Idle: no accent regardless of weather.
    assert_eq!(work_accent_for(WorkWeather::OutputSparks, 0.0), WorkAccent::None);
    // Live work maps weather to accent.
    assert_eq!(work_accent_for(WorkWeather::OutputSparks, 0.9), WorkAccent::Alert);
    assert_eq!(work_accent_for(WorkWeather::ReasoningPulse, 0.9), WorkAccent::Focused);
    assert_eq!(work_accent_for(WorkWeather::CacheMist, 0.9), WorkAccent::Dreamy);
    assert_eq!(work_accent_for(WorkWeather::Mixed, 0.9), WorkAccent::Alert);
    assert_eq!(work_accent_for(WorkWeather::Clear, 0.9), WorkAccent::None);
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib pet::render::tests::work_accent_from_weather_gates_on_live_work`
Expected: FAIL — `work_accent_for` not found.

- [ ] **Step 3: Implement the mapper**

```rust
/// Map live work shape to a subtle expression accent. Returns `None` unless
/// work is actually flowing (activity gate), so a stale weather never lingers
/// on an idle pet — keeping this on the texture side of the locked boundary.
pub fn work_accent_for(weather: crate::tui::life::WorkWeather, activity_level: f32) -> WorkAccent {
    use crate::tui::life::WorkWeather::*;
    if activity_level < 0.3 {
        return WorkAccent::None;
    }
    match weather {
        OutputSparks | Mixed => WorkAccent::Alert,
        ReasoningPulse => WorkAccent::Focused,
        CacheMist => WorkAccent::Dreamy,
        Clear => WorkAccent::None,
    }
}
```

- [ ] **Step 4: Set `work_accent` at the producers**

In `src/commands/watch.rs:81` and `:436` (and `src/tui/layout.rs:471`), set:

```rust
work_accent: crate::pet::render::work_accent_for(
    vm.life_profile.work_weather,
    if vm.life_profile.calm_mode { 0.0 } else { vm.life_profile.activity_level },
),
```

(Calm mode forces no accent, consistent with how activity is already zeroed in calm mode.)

- [ ] **Step 5: Run tests + build + clippy**

Run: `cargo test --lib && cargo build && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 6: Add a preview fixture per work_weather + visually review**

Add (or extend) a live-work fixture per weather class in `src/dev_preview/watch.rs` (reuse `warm_life_profile`/`cooling_life_profile`, vary `work_weather` + `activity_level >= 0.5`). Confirm in the preview that OutputSparks/Reasoning/Cache produce distinct faces and Clear/idle produce none.

- [ ] **Step 7: Commit**

```bash
git add src/commands/watch.rs src/tui/layout.rs src/pet/render.rs src/dev_preview/watch.rs
git commit -m "feat(tui): drive pet expression accent from live work weather"
```

---

## Phase D — Sleep-depth breath + idle laziness

### Task D1: Shallower breath as sleep deepens (TDD)

Deepen sleep by *shortening the inhale window* over time-since-onset (period stays constant, preserving the onset phase anchor — changing the period would break continuity).

**Files:**
- Modify: `src/pet/animator.rs` (`compute_breath_offset_with_rhythm`, Asleep arm)
- Test: inline in `src/pet/animator.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn sleep_breath_gets_shallower_over_time() {
    let onset = time::OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap();
    let inhale_fraction = |elapsed_min: i64| {
        let start = onset + time::Duration::minutes(elapsed_min);
        // Count raised ticks across one ~asleep period window (300 ds is safe).
        (0..300_i64)
            .filter(|ds| {
                compute_breath_offset_with_rhythm(
                    Some(Species::Fuzz),
                    start + time::Duration::milliseconds(ds * 100),
                    BreathRhythm::Asleep { onset },
                ) == 1
            })
            .count()
    };
    let early = inhale_fraction(0);
    let deep = inhale_fraction(60);
    assert!(deep < early, "deep sleep ({deep}) has a shorter inhale than light sleep ({early})");
    assert!(deep > 0, "breath never stops");
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib pet::animator::tests::sleep_breath_gets_shallower_over_time`
Expected: FAIL — inhale window is currently constant (`early == deep`).

- [ ] **Step 3: Implement depth-shortened inhale in the Asleep arm**

In `compute_breath_offset_with_rhythm` (animator.rs:445), reduce `inhale_ds` based on minutes since onset, bounded so it never hits zero:

```rust
BreathRhythm::Asleep { onset } => {
    let elapsed_min = (now - onset).whole_minutes().clamp(0, 90);
    // Inhale window shrinks from full to ~40% over the first 90 min asleep.
    let depth_num = 100 - (elapsed_min * 60 / 90); // 100 -> 40
    let inhale = (inhale_ds * SLEEP_BREATH_INHALE_SCALE * depth_num / 100).max(1);
    (
        period_ds * SLEEP_BREATH_PERIOD_SCALE,
        inhale,
        onset.unix_timestamp() * 10 + i64::from(onset.millisecond() / 100),
    )
}
```

- [ ] **Step 4: Run animator tests, confirm green**

Run: `cargo test --lib pet::animator::`
Expected: PASS (including the C1 sleep test — verify it still holds at onset where `elapsed_min == 0` keeps the full inhale; if its later sample now differs, adjust that sample as in C1 Step 6).

- [ ] **Step 5: Commit**

```bash
git add src/pet/animator.rs
git commit -m "feat(pet): shallower breath as sleep deepens"
```

### Task D2: Idle-gesture laziness (TDD)

As idle grows, slow wander target changes and reduce twinkle frequency. Thread `idle_minutes` into the wander/twinkle computations.

**Files:**
- Modify: `src/pet/animator.rs` (`compute_wander_position_x`, `compute_twinkle`), call sites in `src/tui/panels/pet.rs`
- Test: inline in `src/pet/animator.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn idle_languor_scale_grows_with_idle_minutes() {
    assert_eq!(idle_languor_scale(0), 1.0);
    assert!(idle_languor_scale(30) > idle_languor_scale(0));
    assert!(idle_languor_scale(120) > idle_languor_scale(30));
    assert!(idle_languor_scale(100_000) <= 3.0, "bounded");
}
```

- [ ] **Step 2: Run, confirm failure**

Run: `cargo test --lib pet::animator::tests::idle_languor_scale_grows_with_idle_minutes`
Expected: FAIL — function not found.

- [ ] **Step 3: Implement the scale + apply it**

```rust
/// Multiplier (1.0..=3.0) that stretches idle gesture timing as idle grows:
/// a long-idle pet wanders and sparkles less, reading as settled rather than
/// loop-frozen.
pub fn idle_languor_scale(idle_minutes: u32) -> f64 {
    let m = f64::from(idle_minutes.min(180));
    1.0 + 2.0 * (m / 180.0)
}
```

Apply by multiplying `TARGET_HOLD_SECS` in `compute_wander_position_x` and `TWINKLE_PERIOD_SECS` in `compute_twinkle` by `idle_languor_scale(idle_minutes)`. Add an `idle_minutes: u32` parameter to both functions; pass `vm.life_profile.idle.idle_minutes` from the `src/tui/panels/pet.rs` call sites. Update the existing wander/twinkle tests to pass `0` (preserving current behavior at zero idle).

- [ ] **Step 4: Run tests + clippy**

Run: `cargo test --lib pet::animator:: && cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, clean.

- [ ] **Step 5: Commit**

```bash
git add src/pet/animator.rs src/tui/panels/pet.rs
git commit -m "feat(pet): idle-gesture laziness scales wander and twinkle"
```

---

## Final Task: Full verification + preview gate

**Files:** none (verification only)

- [ ] **Step 1: Full suite**

Run: `cargo test`
Expected: all green.

- [ ] **Step 2: Lint + format gates**

Run: `cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings`
Expected: clean (CI gate).

- [ ] **Step 3: Generate the full preview bundle and review**

Run: `cargo run -- dev-preview --scenario all --out target/glorp-preview && open target/glorp-preview/index.html`
Confirm by eye: dawn/day/dusk/night rooms read distinctly; tired/cozy/asleep pets settle and dim; Ecstatic star-eyes at peak; work accents on live work; breathing natural per species.

- [ ] **Step 4: Tune magnitudes if needed**

If any swing is too bold/subtle (per the "bold room / subtle pet" principle), adjust the constants in `phase_warmth_tint`/`phase_density_scale` (A), `performance_lightness_multiplier`/`performance_posture_offset` (B), the breath table (C1), accent eye strings (E), or the depth/laziness curves (D). Re-run the preview. Commit any tuning separately.

- [ ] **Step 5: Final commit (if tuning happened)**

```bash
git add -p
git commit -m "tune(tui): daily aliveness magnitudes from preview review"
```

---

## Self-Review Checklist (run before handing off)

- [ ] Every spec direction (A/B/C/D/E) maps to tasks above.
- [ ] All `AnimationFrame` construction sites updated for the two new fields.
- [ ] All exhaustive `match mood` arms cover `Ecstatic` (compiler-enforced).
- [ ] C1's two rippled tests (`tired_breath_period_scale_*`, `sleep_breath_is_slower_*`) updated, not deleted.
- [ ] No clock-driven personality: A is texture (palette/density); B/C/D/E key off `PetPerformance`/vitals/`work_weather`/`idle_minutes`/sleep-onset.
- [ ] `work_accent` gated on live activity so it never lingers on an idle pet.
