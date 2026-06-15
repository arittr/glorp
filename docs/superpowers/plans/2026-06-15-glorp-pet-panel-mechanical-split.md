# Glorp PetPanel Mechanical Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `src/tui/panels/pet.rs` into focused child modules without changing rendered output or public behavior.

**Architecture:** Keep `src/tui/panels/pet.rs` as the module root and move pure helper groups into `src/tui/panels/pet/*.rs`. The root keeps `PetPanel`, the `LegacyPanel` implementation, and orchestration of render passes. Child modules expose `pub(super)` helpers used only by the root until later presentation-domain plans migrate them.

**Tech Stack:** Rust 2021, ratatui buffers/layouts, existing `PetPanel` unit tests, `tests/tui_render.rs`, Preview Lab snapshots.

---

## Dependency Gate

- Plan 1 must be committed before this plan starts.
- Run:

```bash
test -f docs/superpowers/plans/2026-06-15-glorp-presentation-contract-freeze.md
cargo test --test dev_preview --features dev-preview dev_preview_watch_and_round_frames_write_scene_artifacts
```

Expected: plan file exists and the contract artifact test passes.

## File Structure

| File | Status | Responsibility |
| --- | --- | --- |
| `tests/pet_panel_structure.rs` | Create | Source-structure guard that proves `pet.rs` remains the module root and child modules exist. |
| `src/tui/panels/pet.rs` | Modify | Keep `PetPanel`, `PET_W`, `PET_H`, `LegacyPanel` orchestration, and re-export/import child helpers. |
| `src/tui/panels/pet/ambient.rs` | Create | Ambient glyphs, motes, weather/activity glyph helpers, silhouette halo helpers. |
| `src/tui/panels/pet/colors.rs` | Create | Palette, tint, lightness, shimmer, prop reaction, and role style helpers. |
| `src/tui/panels/pet/art_lines.rs` | Create | Pet line building, mirroring, role span mapping, cursor-eye helpers, sparse line painting. |
| `src/tui/panels/pet/performance.rs` | Create | Pet performance cue placement and posture/lightness helpers. |
| `src/tui/panels/pet/props.rs` | Create | Prop rendering helpers that call existing `habitat_props_for` and apply prop reaction styles. |

## Forbidden Changes

- Do not move `src/tui/panels/pet.rs` to `src/tui/panels/pet/mod.rs`.
- Do not change public `PetPanel` call sites.
- Do not change rendered pet, room, prop, watch, round, or Preview Lab output.
- Do not introduce `src/presentation/` in this plan.
- Do not move habitat prop placement logic out of `src/tui/component/habitat_props.rs`.

## Task 1: Add Structure Guard

**Files:**
- Create: `tests/pet_panel_structure.rs`

- [ ] **Step 1: Write the failing structure test**

Create `tests/pet_panel_structure.rs`:

```rust
use std::path::Path;

#[test]
fn pet_panel_keeps_root_file_and_child_modules() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let panel_root = root.join("src/tui/panels/pet.rs");
    assert!(panel_root.is_file(), "pet.rs must remain the module root");
    assert!(
        !root.join("src/tui/panels/pet/mod.rs").exists(),
        "do not move the root to pet/mod.rs"
    );

    let source = std::fs::read_to_string(&panel_root).unwrap();
    for module in ["ambient", "colors", "art_lines", "performance", "props"] {
        assert!(
            source.contains(&format!("mod {module};")),
            "pet.rs should declare child module {module}"
        );
        assert!(
            root.join(format!("src/tui/panels/pet/{module}.rs")).is_file(),
            "missing child module file {module}.rs"
        );
    }

    let root_lines = source.lines().count();
    assert!(
        root_lines < 1700,
        "pet.rs should shrink after mechanical split; got {root_lines} lines"
    );
}
```

- [ ] **Step 2: Run the structure test and confirm failure**

Run:

```bash
cargo test --test pet_panel_structure
```

Expected: FAIL because the child modules do not exist.

- [ ] **Step 3: Commit the failing guard**

```bash
git add tests/pet_panel_structure.rs
git commit -m "test: guard PetPanel mechanical split"
```

## Task 2: Create Child Modules and Wire Imports

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Create: `src/tui/panels/pet/ambient.rs`
- Create: `src/tui/panels/pet/colors.rs`
- Create: `src/tui/panels/pet/art_lines.rs`
- Create: `src/tui/panels/pet/performance.rs`
- Create: `src/tui/panels/pet/props.rs`

- [ ] **Step 1: Add module declarations**

Add near the top of `src/tui/panels/pet.rs`, after the existing `use` block:

```rust
mod ambient;
mod art_lines;
mod colors;
mod performance;
mod props;
```

- [ ] **Step 2: Create empty child modules with shared imports**

Create `src/tui/panels/pet/ambient.rs`:

```rust
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
};

use crate::game::evolution::{Stage, Season};
use crate::pet::generation::Species;
use crate::tui::day::{DayContext, DayPhase};
use crate::tui::life::{PetLifeProfile, WorkWeather};
use crate::tui::style::ColorCapability;

use super::{AmbientGlyph, MOTE_BUDGET_SHARE, MOTE_GLYPHS};
```

Create `src/tui/panels/pet/colors.rs`:

```rust
use ratatui::style::{Color, Modifier, Style};

use crate::pet::render::PaletteRoleName;
use crate::tui::day::{DayContext, DayPhase};
use crate::tui::life::{PetLifeProfile, PropReaction, SourceAccent, WorkWeather};
use crate::tui::style::{ColorCapability, SemanticStyles};
```

Create `src/tui/panels/pet/art_lines.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
};

use crate::pet::render::{PaletteRoleName, StyledSegment};
use crate::tui::style::SemanticStyles;
use crate::tui::view_model::WatchViewModel;
```

Create `src/tui/panels/pet/performance.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    style::{Color, Style},
};

use crate::tui::component::PetSceneLayout;
use crate::tui::room::PetPerformance;
use crate::tui::style::ColorCapability;
```

Create `src/tui/panels/pet/props.rs`:

```rust
use ratatui::buffer::Buffer;

use crate::game::habitat::HabitatPetLayer;
use crate::tui::component::{habitat_props_for, HabitatPropCell, PetSceneLayout};
use crate::tui::life::PropReaction;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;
```

- [ ] **Step 3: Run compile and confirm unused-import failures are expected**

Run:

```bash
cargo test --lib tui::panels::pet
```

Expected: FAIL because the modules are empty and imports are unused. Continue immediately to the move tasks before committing.

## Task 3: Move Ambient Helpers

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/panels/pet/ambient.rs`

- [ ] **Step 1: Move ambient functions and constants**

Move these items from `src/tui/panels/pet.rs` to `src/tui/panels/pet/ambient.rs`:

```rust
fn sky_palette_for(species: Species) -> &'static [char]
fn biome_floor_palette(tag: crate::tui::room::RoomBiomeTag) -> &'static [char]
fn biome_floor_fill_percent(tag: crate::tui::room::RoomBiomeTag, phase: DayPhase) -> u16
fn stage_base_count(stage: Stage) -> usize
fn species_seed(species: Species) -> u64
fn stage_seed(stage: Stage) -> u64
fn work_weather_seed(weather: WorkWeather) -> u64
fn phase_count_scale(phase: DayPhase) -> f64
fn season_hue_drift(color: Color, season: Season) -> Color
fn climate_tint(color: Color, climate: Option<WorkWeather>) -> Color
fn sky_palette_for_phase(species: Species, phase: DayPhase, date_seed: u64) -> &'static [char]
fn sky_color_for_phase(species: Species, phase: DayPhase, date_seed: u64, color_capability: ColorCapability) -> Style
pub fn ambient_glyphs_for(...)
pub fn ambient_glyphs_for_phase(...)
fn mote_density(ratio: f32) -> f32
fn mote_glyphs_for(...)
fn effective_weekend_softening(day: &DayContext, profile: &PetLifeProfile) -> f32
fn weekend_soften_color(color: Color, softening: f32) -> Color
fn activity_glyphs_for(...)
pub(crate) fn pet_silhouette_halo_rects(...)
fn ambient_glyph_is_inside_area(...)
```

Keep `AmbientGlyph`, `MOTE_BUDGET_SHARE`, and `MOTE_GLYPHS` in the root for this task, and import them from `super`.

- [ ] **Step 2: Re-export ambient helpers used outside the root**

Add to `src/tui/panels/pet.rs`:

```rust
pub use ambient::{ambient_glyphs_for, ambient_glyphs_for_phase};
pub(crate) use ambient::pet_silhouette_halo_rects;
```

- [ ] **Step 3: Run ambient tests**

Run:

```bash
cargo test --lib ambient_glyphs
cargo test --lib mote_glyphs
```

Expected: PASS.

- [ ] **Step 4: Commit ambient split**

```bash
git add src/tui/panels/pet.rs src/tui/panels/pet/ambient.rs
git commit -m "refactor: split PetPanel ambient helpers"
```

## Task 4: Move Color and Role Style Helpers

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/panels/pet/colors.rs`

- [ ] **Step 1: Move color helpers**

Move these items from `src/tui/panels/pet.rs` to `src/tui/panels/pet/colors.rs`:

```rust
fn activity_glyph_budget(profile: &PetLifeProfile, compact: bool) -> usize
fn activity_glyph_color(profile: &PetLifeProfile) -> Color
fn source_accent_color(accent: SourceAccent) -> Color
fn blend_colors(primary: Color, secondary: Color, primary_weight: f32) -> Color
fn lerp_color(a: Color, b: Color, t: f32) -> Color
fn warm_shift(base: Color, amount: f32) -> Color
fn dim_shift(base: Color, amount: f32) -> Color
fn tint_style_for_phase(style: Style, phase: DayPhase, blend: f32) -> Style
fn cool_shift(color: Color, amount: f32) -> Color
fn tint_pet_styles_for_phase(styles: &SemanticStyles, phase: DayPhase, climate: Option<WorkWeather>) -> SemanticStyles
fn profile_token_pop(profile: &PetLifeProfile, now: i64) -> Option<crate::pet::animator::TokenPop>
fn activity_lift_style(style: Style, profile: &PetLifeProfile) -> Style
fn apply_prop_reaction_style(style: Style, reaction: Option<&PropReaction>, color_capability: ColorCapability) -> Style
fn lift_pet_styles_for_activity(styles: &SemanticStyles, profile: &PetLifeProfile) -> SemanticStyles
fn performance_lightness_multiplier(performance: crate::tui::room::PetPerformance) -> f32
fn performance_posture_offset(performance: crate::tui::room::PetPerformance) -> u16
fn darken_pet_styles(base: &SemanticStyles, multiplier: f32) -> SemanticStyles
fn brighten_pet_role(base: &SemanticStyles, role: Option<PaletteRoleName>, multiplier: f32) -> SemanticStyles
fn brighten_style(style: Style, multiplier: f32) -> Style
fn darken_style(style: Style, multiplier: f32) -> Style
pub(crate) fn pet_role_style(role: PaletteRoleName, palette: &crate::pet::palette::ResolvedPalette) -> Style
fn seed_pet_palette(base: &SemanticStyles, palette: &crate::pet::palette::ResolvedPalette) -> SemanticStyles
fn palette_from_styles(styles: &SemanticStyles) -> crate::pet::palette::ResolvedPalette
```

Use `pub(super)` visibility for helpers called from the root and `pub(crate)` only for `pet_role_style`, preserving the current external test surface.

- [ ] **Step 2: Import color helpers in root**

Add to `src/tui/panels/pet.rs`:

```rust
use colors::{
    activity_glyph_budget, apply_prop_reaction_style, brighten_pet_role, darken_pet_styles,
    lift_pet_styles_for_activity, palette_from_styles, performance_lightness_multiplier,
    performance_posture_offset, profile_token_pop, seed_pet_palette, tint_pet_styles_for_phase,
};
pub(crate) use colors::pet_role_style;
```

- [ ] **Step 3: Run color-focused tests**

Run:

```bash
cargo test --lib pet_role_style
cargo test --lib prop_reaction_style
cargo test --lib weekend_softening
```

Expected: PASS.

- [ ] **Step 4: Commit color split**

```bash
git add src/tui/panels/pet.rs src/tui/panels/pet/colors.rs
git commit -m "refactor: split PetPanel color helpers"
```

## Task 5: Move Pet Art Line Helpers

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/panels/pet/art_lines.rs`

- [ ] **Step 1: Move art line helpers**

Move these items from `src/tui/panels/pet.rs` to `src/tui/panels/pet/art_lines.rs`:

```rust
fn cursor_normalized_x_within(vm: &WatchViewModel, area: Rect) -> Option<f32>
fn cursor_eye_glyph(norm_x: f32) -> char
fn build_cursor_eye_string(glyph: char, span_width: usize) -> String
fn build_pet_lines(...)
pub(crate) fn mirror_line(line: &str) -> String
fn mirror_char(c: char) -> char
fn mirror_spans(spans: &[StyledSegment], line_width: usize) -> Vec<StyledSegment>
fn build_owned_spans_for_line(...)
fn apply_twinkle_in_range(...)
pub(crate) fn pet_role_spans_for_line(...)
fn char_byte_indices(line: &str) -> Vec<usize>
fn char_slice<'a>(line: &'a str, indices: &[usize], start_char: usize, end_char: usize) -> &'a str
fn render_pet_lines_sparse(buf: &mut Buffer, area: Rect, lines: &[Line<'_>])
fn render_speech_bubble(area: Rect, buf: &mut Buffer, text: &str, styles: &SemanticStyles)
```

Keep calls to `colors::pet_role_style` through `super::pet_role_style`.

- [ ] **Step 2: Re-export public crate helpers**

Add to `src/tui/panels/pet.rs`:

```rust
pub(crate) use art_lines::{mirror_line, pet_role_spans_for_line};
use art_lines::{build_pet_lines, cursor_normalized_x_within, render_pet_lines_sparse, render_speech_bubble};
```

- [ ] **Step 3: Run pet-art tests**

Run:

```bash
cargo test --lib build_pet_lines
cargo test --lib mirror_line
cargo test --test dev_preview --features dev-preview dev_preview_pets_writes_species_stage_matrix
```

Expected: PASS.

- [ ] **Step 4: Commit art split**

```bash
git add src/tui/panels/pet.rs src/tui/panels/pet/art_lines.rs
git commit -m "refactor: split PetPanel art line helpers"
```

## Task 6: Move Performance and Prop Pass Helpers

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/panels/pet/performance.rs`
- Modify: `src/tui/panels/pet/props.rs`

- [ ] **Step 1: Move performance helpers**

Move these items to `src/tui/panels/pet/performance.rs`:

```rust
fn apply_pet_performance_cues(...)
fn performance_cue_style(color_capability: ColorCapability) -> Style
fn mark_pet_floor(buf: &mut Buffer, scene: &PetSceneLayout, symbol: char, style: Style)
fn mark_pet_air(buf: &mut Buffer, scene: &PetSceneLayout, symbol: char, style: Style)
```

Import in `src/tui/panels/pet.rs`:

```rust
use performance::apply_pet_performance_cues;
```

- [ ] **Step 2: Move prop pass helpers**

Move these items to `src/tui/panels/pet/props.rs`:

```rust
fn habitat_contains(scene: &PetSceneLayout, prop: &HabitatPropCell) -> bool
```

Add this new helper in `src/tui/panels/pet/props.rs` so the root stops duplicating the behind/foreground pass loops:

```rust
pub(super) fn render_prop_layer(
    buf: &mut Buffer,
    prop_cells: &[HabitatPropCell],
    scene: &PetSceneLayout,
    reactions: &[PropReaction],
    color_capability: ColorCapability,
    layer: HabitatPetLayer,
) {
    for prop in prop_cells {
        if prop.pet_layer == layer && habitat_contains(scene, prop) {
            let reaction = reactions
                .iter()
                .find(|reaction| reaction.prop_id == prop.prop_id);
            let cell = &mut buf[(prop.col, prop.row)];
            cell.set_char(prop.glyph);
            cell.set_style(super::apply_prop_reaction_style(
                prop.style,
                reaction,
                color_capability,
            ));
        }
    }
}
```

- [ ] **Step 3: Replace root prop loops**

In `PetPanel::render`, replace the repeated prop loops with:

```rust
props::render_prop_layer(
    buf,
    &prop_cells,
    &scene,
    &life_profile.prop_reactions,
    ctx.color_capability,
    HabitatPetLayer::Background,
);
props::render_prop_layer(
    buf,
    &prop_cells,
    &scene,
    &life_profile.prop_reactions,
    ctx.color_capability,
    HabitatPetLayer::Behind,
);
```

Keep the pet render call between behind and foreground passes. Render foreground after pet:

```rust
props::render_prop_layer(
    buf,
    &prop_cells,
    &scene,
    &life_profile.prop_reactions,
    ctx.color_capability,
    HabitatPetLayer::Foreground,
);
```

- [ ] **Step 4: Run performance and prop tests**

Run:

```bash
cargo test --lib performance_cue
cargo test --lib habitat_props_for
cargo test --test tui_render
```

Expected: PASS.

- [ ] **Step 5: Commit performance and prop split**

```bash
git add src/tui/panels/pet.rs src/tui/panels/pet/performance.rs src/tui/panels/pet/props.rs
git commit -m "refactor: split PetPanel performance and prop passes"
```

## Task 7: Final Behavior Verification

**Files:**
- No edits.

- [ ] **Step 1: Run PetPanel structure guard**

```bash
cargo test --test pet_panel_structure
```

Expected: PASS.

- [ ] **Step 2: Run focused pet and render tests**

```bash
cargo test --lib tui::panels::pet
cargo test --test tui_render
cargo test --test dev_preview --features dev-preview
```

Expected: PASS.

- [ ] **Step 3: Regenerate Preview Lab**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
```

Expected: exits 0. Existing `frames/*.txt`, `frames/*.cells.json`, `frames/*.layout.json`, `frames/*.room.txt`, `frames/*.room-masked.txt`, and `strips/**` paths still exist.

- [ ] **Step 4: Run repository checks**

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --features dev-preview
git status --short --branch
```

Expected: all checks pass and git status is clean after the final commit.

## Stop Conditions

- Stop if a visual snapshot changes for a reason other than module movement.
- Stop if the split requires broad public API changes outside `src/tui/panels/pet.rs` and its child modules.
- Stop if the root still exceeds 1700 lines after the planned moves.
- Stop if circular imports force moving `PetPanel` orchestration into a child module.

