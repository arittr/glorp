# Glorp Style Pass — Tranche 2 (Environments + Companion) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make environments read as distinct *places* across the watch and the macOS companion — biome-keyed floor + subtle background wash + strengthened night + split dialects on the watch, a per-pet day/night ambient "light" layer, and a companion that finally renders a biome-tinted background and real room texture.

**Architecture:** The watch room generator (`room.rs::room_glyphs_for`) already splits **selection** (the pure functions `biome_symbols`, `biome_style`, `zone_counts_for_biome`, `dialect_zone_counts`) from **placement** (`place_zone_glyphs`/`zone_rect`). So "shared generator" is realized by **exposing those selection atoms `pub(crate)` and having the companion reuse them with its own circular-aperture placement** — NOT by the byte-identical refactor of `room_glyphs_for` the spec sketched (the re-grounding showed that refactor is unnecessary and risky; the atoms are already factored out and keep internal callers, so the watch path is untouched and stays byte-identical for free). Color/symbol values are concrete starting points, tuned later in color review.

**Tech Stack:** Rust 2021, ratatui 0.29, objc2/AppKit (companion). No new deps.

**Spec:** `docs/superpowers/specs/2026-06-13-glorp-style-pass-design.md` (Tranche 2). This plan **re-grounds** the spec's environment section against current code: night already exists (just subtle), the species-keyed floor lives in `pet.rs::ambient_glyphs_for_phase` (a separate painter from `room_glyphs_for`), and there is a single flat theme bg but no per-habitat wash.

**Deviations from spec, decided here (documented for review):**
1. **No `room_glyphs_for` selection/placement refactor or byte-identical golden.** Instead expose the already-pure selection atoms and reuse them in the companion. Lower risk, same outcome (companion room uses the real biome vocabulary).
2. **Floor re-key is watch-only** (`floor_palette_for` → biome). The companion has no "floor row" (circular aperture); it gets biome texture from the shared atoms scattered in the aperture. (This is the floor fork from brainstorming, resolved toward the simpler option.)

**Scope note:** Color/symbol/intensity values (wash colors, night scales, dialect symbol sets, ambient tint amounts) are concrete starting values, reviewed/tuned in color via `index.html` after this lands. Weather-as-motif remains deferred. Structural behavior is TDD-tested; exact colors are not pinned in tests.

---

## File Structure

- **Modify** `src/tui/room.rs` — expose selection atoms `pub(crate)`; split the four Default dialects in `biome_symbols`.
- **Modify** `src/tui/panels/pet.rs` — biome-key the floor (`ambient_glyphs_for_phase` gains a biome arg + `biome_floor_palette`); add the per-habitat background wash pass in `PetPanel::render`; strengthen night scales; add `tint_pet_styles_for_phase` in `render_pet_inside`.
- **Modify** `src/companion/render.rs` — biome-tinted `biome_background_color`; emit `RoomGlyph` draw commands from shared atoms.
- **Modify** `src/companion/app.rs` — implement the live `RoomGlyph` draw arm; day-phase background darkening.
- **Modify** `src/round/preview.rs` — `paint_room`/`room_symbol` use the shared biome vocabulary (biome+dialect), not the dialect-only `#/^/.` lattice.
- **Modify** `src/round/model.rs` — (if needed) ensure `RoundRoomModel` exposes what the companion room painter needs (already has `biome`, `dialect`, `work_weather`, `day_phase`).

---

## Tranche 2a — Shared selection vocabulary + dialect split

### Task 1: Expose the room selection atoms `pub(crate)`

The companion will reuse these. They already have internal callers in `biome_glyphs` (room.rs:786-808), so exposing them creates no dead code.

**Files:**
- Modify: `src/tui/room.rs` (`biome_symbols` ~592, `biome_style` ~770, `zone_counts_for_biome` ~637, `dialect_zone_counts` ~626)

- [ ] **Step 1: Write the failing test**

In `src/tui/room.rs` test module, add:

```rust
    #[test]
    fn selection_atoms_are_crate_visible() {
        // Compile-time proof these are reusable by other modules (e.g. the companion).
        let dialect = RoomSpeciesDialect::for_species(crate::pet::generation::Species::Crystal);
        let syms = biome_symbols(RoomBiomeTag::Celestial, dialect);
        assert!(!syms.is_empty());
        let _style = biome_style(RoomBiomeTag::Celestial, ColorCapability::Truecolor);
    }
```

- [ ] **Step 2: Run to verify it compiles/passes now (guard) or fails on visibility**

Run: `cargo test --lib tui::room::tests::selection_atoms_are_crate_visible`
Expected: PASS (the functions are in-module). This test pins their existence; the visibility change in Step 3 keeps it green while making them reusable cross-module.

- [ ] **Step 3: Change visibility**

In `src/tui/room.rs`, change these four signatures from `fn` to `pub(crate) fn`:
- `fn biome_symbols(` → `pub(crate) fn biome_symbols(`
- `fn biome_style(` → `pub(crate) fn biome_style(`
- `fn zone_counts_for_biome(` → `pub(crate) fn zone_counts_for_biome(`
- `fn dialect_zone_counts(` → `pub(crate) fn dialect_zone_counts(`

Also ensure `RoomGlyph`, `RoomZone`, `zone_rect`, and `place_zone_glyphs` are reachable if the companion later wants placement helpers — but do NOT expose those yet (YAGNI; the companion does its own aperture placement). Leave them private.

- [ ] **Step 4: Run tests + clippy**

Run: `cargo test --lib tui::room` then `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS, no dead-code warnings (internal callers remain).

- [ ] **Step 5: Commit**

```bash
git add src/tui/room.rs
git commit -m "refactor: expose room biome-selection atoms pub(crate) for reuse"
```

### Task 2: Split the four "Default" dialects in `biome_symbols`

Today Fuzz/Blob/Ghost/Mech share one symbol arm (room.rs:610-620). Give each its own family so same-biome rooms differ by species. Glitch/Crystal (Tuned) stay unchanged. Values are starting points (tune in review).

**Files:**
- Modify: `src/tui/room.rs` (`biome_symbols` ~592-622)

- [ ] **Step 1: Write the failing test**

In `src/tui/room.rs` test module, add:

```rust
    #[test]
    fn default_dialects_have_distinct_symbol_families() {
        use crate::pet::generation::Species;
        let tag = RoomBiomeTag::Botanical;
        let fuzz = biome_symbols(tag, RoomSpeciesDialect::for_species(Species::Fuzz));
        let blob = biome_symbols(tag, RoomSpeciesDialect::for_species(Species::Blob));
        let ghost = biome_symbols(tag, RoomSpeciesDialect::for_species(Species::Ghost));
        let mech = biome_symbols(tag, RoomSpeciesDialect::for_species(Species::Mech));
        // No two of the four Default dialects share an identical symbol set.
        let sets = [fuzz, blob, ghost, mech];
        for i in 0..sets.len() {
            for j in (i + 1)..sets.len() {
                assert_ne!(sets[i], sets[j], "dialects {i} and {j} share symbols");
            }
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tui::room::tests::default_dialects_have_distinct_symbol_families`
Expected: FAIL (all four currently return the same `match tag` arm).

- [ ] **Step 3: Replace the shared Default arm**

In `biome_symbols`, replace the combined arm:

```rust
        RoomDialectKey::Fuzz
        | RoomDialectKey::Blob
        | RoomDialectKey::Ghost
        | RoomDialectKey::Mech => match tag {
            RoomBiomeTag::Starter => &['.', '·'],
            RoomBiomeTag::Botanical => &['"', '\'', '`', ','],
            RoomBiomeTag::Technical => &[':', ';', '+', '='],
            RoomBiomeTag::Celestial => &['*', '·', '˚', '.'],
            RoomBiomeTag::Artifact => &['.', 'o', '◇', '°'],
            RoomBiomeTag::Cozy => &['~', '·', '⌞', '⌟'],
        },
```

with four distinct arms (Fuzz = soft fur motes; Blob = rounded bubbles; Ghost = misty/sparse; Mech = gridded ticks):

```rust
        RoomDialectKey::Fuzz => match tag {
            RoomBiomeTag::Starter => &['·', '.'],
            RoomBiomeTag::Botanical => &['\'', '`', ',', '·'],
            RoomBiomeTag::Technical => &['·', ':', '\'', '.'],
            RoomBiomeTag::Celestial => &['·', '\'', '˚', '.'],
            RoomBiomeTag::Artifact => &['·', '.', '∘', '°'],
            RoomBiomeTag::Cozy => &['·', '\'', ',', '~'],
        },
        RoomDialectKey::Blob => match tag {
            RoomBiomeTag::Starter => &['.', '°'],
            RoomBiomeTag::Botanical => &['°', 'o', '·', ','],
            RoomBiomeTag::Technical => &['°', 'o', ':', '·'],
            RoomBiomeTag::Celestial => &['°', 'o', '∘', '·'],
            RoomBiomeTag::Artifact => &['o', '°', '∘', '·'],
            RoomBiomeTag::Cozy => &['~', '°', 'o', '·'],
        },
        RoomDialectKey::Ghost => match tag {
            RoomBiomeTag::Starter => &['\'', ' ', '·'],
            RoomBiomeTag::Botanical => &['\'', '`', ' ', '·'],
            RoomBiomeTag::Technical => &['\'', ':', ' ', '·'],
            RoomBiomeTag::Celestial => &['˚', '\'', ' ', '·'],
            RoomBiomeTag::Artifact => &['\'', '°', ' ', '·'],
            RoomBiomeTag::Cozy => &['~', '\'', ' ', '·'],
        },
        RoomDialectKey::Mech => match tag {
            RoomBiomeTag::Starter => &['·', '─'],
            RoomBiomeTag::Botanical => &['┄', '·', ',', '─'],
            RoomBiomeTag::Technical => &['─', '┄', '╌', '·'],
            RoomBiomeTag::Celestial => &['·', '°', '─', '˚'],
            RoomBiomeTag::Artifact => &['─', '·', '□', '°'],
            RoomBiomeTag::Cozy => &['─', '·', '┄', '~'],
        },
```

(Keep the existing `RoomDialectKey::Glitch` and `RoomDialectKey::Crystal` arms unchanged.)

- [ ] **Step 4: Run the test + full room suite**

Run: `cargo test --lib tui::room` then `cargo test --test dev_preview`
Expected: PASS. If `dev_preview` has a `watch-species-dialect-*` snapshot, it may legitimately change for Fuzz/Blob/Ghost/Mech — verify the diff is only symbol-family changes for those dialects (use `INSTA_UPDATE=always cargo test --test dev_preview`, then `git diff tests/snapshots/` to confirm only Default-dialect room frames changed), and stage the updated snapshots.

- [ ] **Step 5: Commit**

```bash
git add src/tui/room.rs tests/snapshots/
git commit -m "feat: give each Default-dialect species its own room symbol family"
```

---

## Tranche 2b — Watch environment

### Task 3: Biome-key the floor row

The floor row in `ambient_glyphs_for_phase` (pet.rs:562-575) uses `floor_palette_for(species)` (pet.rs:89-99). Re-key it to biome. `ambient_glyphs_for_phase` gains a `RoomBiome` arg; the call site (pet.rs:909) already has `room_profile.biome` in scope.

**Files:**
- Modify: `src/tui/panels/pet.rs` (`floor_palette_for` ~89, `ambient_glyphs_for_phase` signature ~491 + floor line ~524, call site ~909-920)

- [ ] **Step 1: Write the failing test**

In `src/tui/panels/pet.rs` test module, add:

```rust
    #[test]
    fn floor_palette_is_biome_keyed() {
        use crate::tui::room::RoomBiomeTag;
        let botanical = biome_floor_palette(RoomBiomeTag::Botanical);
        let technical = biome_floor_palette(RoomBiomeTag::Technical);
        let artifact = biome_floor_palette(RoomBiomeTag::Artifact);
        assert_ne!(botanical, technical);
        assert_ne!(technical, artifact);
        assert_ne!(botanical, artifact);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tui::panels::pet::tests::floor_palette_is_biome_keyed`
Expected: FAIL (`biome_floor_palette` undefined).

- [ ] **Step 3: Add `biome_floor_palette` and thread biome into the floor**

In `src/tui/panels/pet.rs`, add (near `floor_palette_for`):

```rust
/// Per-biome floor-glyph palette — the ground texture under the pet, keyed to
/// the earned biome rather than species so the floor reads as a place.
fn biome_floor_palette(tag: crate::tui::room::RoomBiomeTag) -> &'static [char] {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => &['·', '.', ' ', ' '],
        RoomBiomeTag::Botanical => &[',', '·', '"', '.', ' '],
        RoomBiomeTag::Technical => &['─', '┄', '·', '.', ' '],
        RoomBiomeTag::Celestial => &['·', '˚', '.', ' ', ' '],
        RoomBiomeTag::Artifact => &['◦', '·', '°', '.', ' '],
        RoomBiomeTag::Cozy => &['·', '~', ',', '.', ' '],
    }
}
```

Change `ambient_glyphs_for_phase`'s signature to accept the biome (add `biome: crate::tui::room::RoomBiomeTag,` after `stage: Stage,`), and replace `let floor = floor_palette_for(species);` (pet.rs:524) with `let floor = biome_floor_palette(biome);`. If `floor_palette_for` becomes unused, remove it (clippy will flag it).

- [ ] **Step 4: Update the call site**

At the `ambient_glyphs_for_phase(` call in `PetPanel::render` (pet.rs:909), pass the biome. `room_profile` is computed at pet.rs:888 (`derive_room_life_profile`), so add `room_profile.biome.primary,` as the new second argument (after `species,`/before `stage,` per the signature you defined — keep argument order consistent with the signature).

- [ ] **Step 5: Run tests**

Run: `cargo test --lib tui::panels::pet` then `cargo test`
Expected: PASS. The watch-wide snapshot floor row will change (species→biome glyphs) — update via `INSTA_UPDATE=always cargo test --test dev_preview`, confirm `git diff tests/snapshots/` shows only floor-row glyph changes, stage the snapshot.

- [ ] **Step 6: Commit**

```bash
git add src/tui/panels/pet.rs tests/snapshots/
git commit -m "feat: key the watch floor row to biome instead of species"
```

### Task 4: Per-biome subtle background wash

Add a whisper-quiet per-biome bg behind the habitat. Cells start with the flat theme bg (`p.bg.rgb`); we set a slightly biome-tinted bg on each habitat cell as a base pass, BEFORE the room glyphs (which set fg only).

**Files:**
- Modify: `src/tui/panels/pet.rs` (add `biome_wash_color`; insert a base pass in `PetPanel::render` just before the room-glyph pass at ~888)

- [ ] **Step 1: Write the failing test**

In `src/tui/panels/pet.rs` test module, add:

```rust
    #[test]
    fn biome_wash_is_subtle_and_biome_distinct() {
        use crate::tui::room::RoomBiomeTag;
        use ratatui::style::Color;
        let base = crate::tui::style::tokenpet_palette().bg.rgb;
        let Color::Rgb(br, bg_, bb) = base else { panic!("bg is rgb") };
        let bot = biome_wash_color(RoomBiomeTag::Botanical);
        let tech = biome_wash_color(RoomBiomeTag::Technical);
        assert_ne!(bot, tech, "biomes must wash differently");
        // Subtle: each channel within 24 of the base theme bg.
        if let Color::Rgb(r, g, b) = bot {
            assert!((r as i16 - br as i16).abs() <= 24);
            assert!((g as i16 - bg_ as i16).abs() <= 24);
            assert!((b as i16 - bb as i16).abs() <= 24);
        } else {
            panic!("wash must be rgb");
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tui::panels::pet::tests::biome_wash_is_subtle_and_biome_distinct`
Expected: FAIL (`biome_wash_color` undefined).

- [ ] **Step 3: Add `biome_wash_color`**

In `src/tui/panels/pet.rs`, add:

```rust
/// A whisper-quiet per-biome background wash: the theme bg nudged a few points
/// toward the biome's hue, so the habitat reads as a place even in a screenshot
/// without overpowering the pet/panels.
fn biome_wash_color(tag: crate::tui::room::RoomBiomeTag) -> ratatui::style::Color {
    use crate::tui::room::RoomBiomeTag;
    use ratatui::style::Color;
    let Color::Rgb(r, g, b) = crate::tui::style::tokenpet_palette().bg.rgb else {
        return crate::tui::style::tokenpet_palette().bg.rgb;
    };
    // Small signed nudges per channel (kept within +-16 so it stays subtle).
    let (dr, dg, db): (i16, i16, i16) = match tag {
        RoomBiomeTag::Starter => (0, 0, 0),
        RoomBiomeTag::Botanical => (-2, 8, -2),
        RoomBiomeTag::Technical => (-2, 2, 12),
        RoomBiomeTag::Celestial => (2, 2, 10),
        RoomBiomeTag::Artifact => (10, 4, -4),
        RoomBiomeTag::Cozy => (10, 2, -2),
    };
    let clamp = |v: i16| v.clamp(0, 255) as u8;
    Color::Rgb(
        clamp(r as i16 + dr),
        clamp(g as i16 + dg),
        clamp(b as i16 + db),
    )
}
```

- [ ] **Step 4: Insert the base wash pass in `PetPanel::render`**

Immediately before the room-glyph composition (the `let room_profile = ...; let room_glyphs = ...` block at pet.rs:888), add a base pass that sets the wash bg on every habitat cell:

```rust
        // Base layer: a subtle per-biome background wash over the habitat, so the
        // room reads as a place. Set BEFORE room/ambient glyphs (which set fg only,
        // leaving this bg intact underneath).
        {
            let room_profile_for_wash = crate::tui::room::derive_room_life_profile(vm, now);
            let wash = biome_wash_color(room_profile_for_wash.biome.primary);
            for wy in scene.habitat.y..scene.habitat.y.saturating_add(scene.habitat.height) {
                for wx in scene.habitat.x..scene.habitat.x.saturating_add(scene.habitat.width) {
                    if !rects_contain(&ambient_exclusions, wx, wy) {
                        buf[(wx, wy)].set_style(ratatui::style::Style::default().bg(wash));
                    }
                }
            }
        }
```

Note: `derive_room_life_profile` is already called again just below for the room glyphs (pet.rs:888); to avoid computing it twice, hoist the existing `let room_profile = derive_room_life_profile(vm, now);` above this wash block and reuse it (use `room_profile.biome.primary` for the wash and pass `&room_profile` to `room_glyphs_for`). Confirm `ambient_exclusions` is in scope at this point (it is used by the room pass just below).

- [ ] **Step 5: Run tests**

Run: `cargo test --lib tui::panels::pet` then `cargo test`
Expected: PASS. Watch snapshots gain a bg on habitat cells — update via `INSTA_UPDATE=always cargo test --test dev_preview`, confirm `git diff tests/snapshots/` shows only added `background-color`/bg on habitat cells, stage.

- [ ] **Step 6: Commit**

```bash
git add src/tui/panels/pet.rs tests/snapshots/
git commit -m "feat: add a subtle per-biome background wash to the watch habitat"
```

### Task 5: Strengthen the night swing

Night exists but is too subtle. Deepen `phase_density_scale` (room.rs:540), `phase_count_scale` (pet.rs:305), and the night sky/floor dim so the same biome reads clearly different at night.

**Files:**
- Modify: `src/tui/room.rs` (`phase_density_scale` ~540)
- Modify: `src/tui/panels/pet.rs` (`phase_count_scale` ~305, `sky_color_for_phase` Night arm ~367, floor Night dim ~531)

- [ ] **Step 1: Write the failing test**

In `src/tui/room.rs` test module, add:

```rust
    #[test]
    fn night_density_is_clearly_lower_than_day() {
        // Night should drop to <= 40% of day density (was 50%, too subtle).
        assert!(phase_density_scale(DayPhase::Night) <= 0.40);
        assert!(phase_density_scale(DayPhase::Night) < phase_density_scale(DayPhase::Dusk));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tui::room::tests::night_density_is_clearly_lower_than_day`
Expected: FAIL (current Night = 0.5).

- [ ] **Step 3: Deepen the scales**

In `src/tui/room.rs` `phase_density_scale`, change `DayPhase::Night => 0.5,` to `DayPhase::Night => 0.35,`.

In `src/tui/panels/pet.rs` `phase_count_scale`, change `DayPhase::Night => 0.6,` to `DayPhase::Night => 0.45,`.

In `src/tui/panels/pet.rs` `sky_color_for_phase`, change `DayPhase::Night => dim_shift(base, 0.40),` to `DayPhase::Night => dim_shift(base, 0.58),`.

In `src/tui/panels/pet.rs` `ambient_glyphs_for_phase` floor dim (pet.rs:531-535), change `dim_shift(base, 0.40 * phase_blend)` to `dim_shift(base, 0.55 * phase_blend)`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib tui::room` then `cargo test`
Expected: PASS. Update any night snapshot (`watch-daycontext-night-asleep`) via `INSTA_UPDATE=always cargo test --test dev_preview`; confirm the diff only deepens dimming/sparsity; stage.

- [ ] **Step 5: Commit**

```bash
git add src/tui/room.rs src/tui/panels/pet.rs tests/snapshots/
git commit -m "feat: deepen the night swing (sparser + dimmer + cooler)"
```

### Task 6: Per-pet day/night ambient ("light") layer

Add a phase tint applied to the per-pet palette so the pet itself reads warmer at dusk and cooler/dimmer at night — the "light" half of pigment-vs-light. Hook it into `render_pet_inside` right after `seed_pet_palette` (pet.rs:1152), reusing `warm_shift`/`dim_shift`.

**Files:**
- Modify: `src/tui/panels/pet.rs` (`render_pet_inside` ~1144, add `tint_pet_styles_for_phase`)

- [ ] **Step 1: Write the failing test**

In `src/tui/panels/pet.rs` test module, add:

```rust
    #[test]
    fn phase_tint_cools_pet_at_night() {
        use crate::tui::day::DayPhase;
        use ratatui::style::{Color, Style};
        let day = Style::default().fg(Color::Rgb(0xc0, 0xa0, 0x60));
        let night = tint_style_for_phase(day, DayPhase::Night, 1.0);
        let (Color::Rgb(_, _, db), Color::Rgb(_, _, nb)) = (day.fg.unwrap(), night.fg.unwrap())
        else {
            panic!("rgb");
        };
        // Night dims overall; assert it changed and is not brighter than day on red.
        assert_ne!(day.fg, night.fg, "night must retint");
        let Color::Rgb(dr, _, _) = day.fg.unwrap() else { panic!() };
        let Color::Rgb(nr, _, _) = night.fg.unwrap() else { panic!() };
        assert!(nr <= dr, "night should not warm/brighten red channel");
        let _ = (db, nb);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib tui::panels::pet::tests::phase_tint_cools_pet_at_night`
Expected: FAIL (`tint_style_for_phase` undefined).

- [ ] **Step 3: Add the phase tint helpers**

In `src/tui/panels/pet.rs`, add:

```rust
/// Apply the day-phase "ambient light" to one style's fg: warmer at dusk,
/// cooler and dimmer at night, neutral by day. Mirrors the sky's phase curve
/// (warm_shift/dim_shift) so pet and room share one light.
fn tint_style_for_phase(style: Style, phase: DayPhase, blend: f32) -> Style {
    let Some(fg) = style.fg else { return style };
    let Color::Rgb(..) = fg else { return style };
    let tinted = match phase {
        DayPhase::Day => fg,
        DayPhase::Dawn => warm_shift(fg, 0.10 * blend),
        DayPhase::Dusk => warm_shift(fg, 0.18 * blend),
        DayPhase::Night => dim_shift(cool_shift(fg, 0.18 * blend), 0.28 * blend),
    };
    style.fg(tinted)
}

/// Nudge a color toward cool (more blue, less red) for night ambience.
fn cool_shift(color: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = color else { return color };
    let amt = amount.clamp(0.0, 1.0);
    let r2 = (f32::from(r) * (1.0 - 0.5 * amt)).round() as u8;
    let b2 = (f32::from(b) + (255.0 - f32::from(b)) * 0.25 * amt).round() as u8;
    Color::Rgb(r2, g, b2)
}

/// Apply the phase tint to all five pet roles of a SemanticStyles.
fn tint_pet_styles_for_phase(styles: &SemanticStyles, phase: DayPhase, blend: f32) -> SemanticStyles {
    let mut s = styles.clone();
    s.pet_body = tint_style_for_phase(s.pet_body, phase, blend);
    s.pet_eye = tint_style_for_phase(s.pet_eye, phase, blend);
    s.pet_mouth = tint_style_for_phase(s.pet_mouth, phase, blend);
    s.pet_accent = tint_style_for_phase(s.pet_accent, phase, blend);
    s.pet_pattern = tint_style_for_phase(s.pet_pattern, phase, blend);
    s
}
```

If `warm_shift`/`dim_shift` are not visible at this location, they are defined earlier in the same file (pet.rs:241-262); confirm scope.

- [ ] **Step 4: Hook it into `render_pet_inside`**

In `render_pet_inside` (pet.rs:1152), after `let base = seed_pet_palette(&semantic_styles(), &vm.pet_palette);`, wrap with the phase tint. Compute the phase blend the same way `PetPanel::render` does (pet.rs:905-908):

```rust
        let base = seed_pet_palette(&semantic_styles(), &vm.pet_palette);
        let phase_blend = {
            let since = (now - vm.day_context.phase_started_at_utc).whole_seconds() as f32;
            (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
        };
        let base = tint_pet_styles_for_phase(&base, vm.day_context.day_phase, phase_blend);
```

(`render_pet_inside` already receives `now` and `vm`, so `vm.day_context` is in scope.)

- [ ] **Step 5: Run tests**

Run: `cargo test --lib tui::panels::pet` then `cargo test`
Expected: PASS. The night/asleep watch snapshot pet now reads cooler/dimmer — update via `INSTA_UPDATE=always cargo test --test dev_preview`, confirm the diff only retints pet cells under night, stage.

- [ ] **Step 6: Commit**

```bash
git add src/tui/panels/pet.rs tests/snapshots/
git commit -m "feat: tint the pet by day-phase ambient light (warm dusk, cool night)"
```

---

## Tranche 2c — Companion

### Task 7: Biome-tinted companion background

The live + preview companion uses a single dark `BACKGROUND_COLOR`. Make it biome-aware. `scene.room.biome` is available.

**Files:**
- Modify: `src/companion/render.rs` (`BACKGROUND_COLOR` ~32, Background push ~51)

- [ ] **Step 1: Write the failing test**

In `src/companion/render.rs` test module, add:

```rust
    #[test]
    fn background_is_biome_tinted() {
        use crate::tui::room::RoomBiomeTag;
        let botanical = biome_background_color(RoomBiomeTag::Botanical);
        let technical = biome_background_color(RoomBiomeTag::Technical);
        assert_ne!(botanical, technical);
        // Stays dark (each rgb channel <= 0.22) so the pet pops.
        assert!(botanical.0 <= 0.22 && botanical.1 <= 0.22 && botanical.2 <= 0.22);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib companion::render::tests::background_is_biome_tinted`
Expected: FAIL (`biome_background_color` undefined).

- [ ] **Step 3: Add `biome_background_color` and use it**

In `src/companion/render.rs`, add:

```rust
/// A dark, biome-tinted background for the companion aperture — keeps the pet
/// dominant (all channels stay low) while giving each place its own cast.
pub(crate) fn biome_background_color(tag: crate::tui::room::RoomBiomeTag) -> RoundColor {
    use crate::tui::room::RoomBiomeTag;
    match tag {
        RoomBiomeTag::Starter => RoundColor(0.08, 0.09, 0.10, 1.0),
        RoomBiomeTag::Botanical => RoundColor(0.07, 0.11, 0.08, 1.0),
        RoomBiomeTag::Technical => RoundColor(0.07, 0.09, 0.13, 1.0),
        RoomBiomeTag::Celestial => RoundColor(0.08, 0.08, 0.14, 1.0),
        RoomBiomeTag::Artifact => RoundColor(0.12, 0.10, 0.07, 1.0),
        RoomBiomeTag::Cozy => RoundColor(0.13, 0.09, 0.08, 1.0),
    }
}
```

In `build_draw_commands`, change the Background command's `color: BACKGROUND_COLOR,` (render.rs:51) to `color: biome_background_color(scene.room.biome.primary),`. Remove `BACKGROUND_COLOR` if it becomes unused (clippy will flag), or keep it as the `Starter` value referenced by the function.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib companion` then `cargo test --test dev_preview`
Expected: PASS. Round preview frames' background may change; update snapshots if any pin it (`INSTA_UPDATE=always`), verify diff is only bg color.

- [ ] **Step 5: Commit**

```bash
git add src/companion/render.rs tests/snapshots/
git commit -m "feat: biome-tint the companion background"
```

### Task 8: Emit + draw companion `RoomGlyph` from the shared vocabulary

Make the companion render real room texture: emit `RoomGlyph` draw commands scattered in the aperture using the shared `biome_symbols`/`biome_style`, and implement the live no-op arm.

**Files:**
- Modify: `src/companion/render.rs` (`build_draw_commands` ~39-90: emit RoomGlyph commands)
- Modify: `src/companion/app.rs` (`draw_command` RoomGlyph arm ~410)

- [ ] **Step 1: Write the failing test**

In `src/companion/render.rs` test module, add:

```rust
    #[test]
    fn emits_room_glyphs_inside_the_aperture() {
        use crate::round::layout::{layout_round_scene, RoundAperture, RoundRenderCapabilities};
        use crate::round::model::derive_round_scene_model;
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        let vm = WatchViewModel::fixture_with_habitat_props();
        let scene = derive_round_scene_model(&vm, datetime!(2026-06-14 12:00 UTC));
        let layout = layout_round_scene(
            &scene,
            RoundAperture::new(52, 52),
            RoundRenderCapabilities::preview_truecolor(),
        );
        let commands = build_draw_commands(&scene, &layout);
        let room: Vec<_> = commands
            .iter()
            .filter(|c| c.kind == RoundDrawKind::RoomGlyph)
            .collect();
        assert!(!room.is_empty(), "companion should emit room glyphs");
        for c in &room {
            assert!(layout.aperture.contains(c.x, c.y), "room glyph outside aperture");
            assert!(c.label.is_some(), "room glyph needs a char");
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib companion::render::tests::emits_room_glyphs_inside_the_aperture`
Expected: FAIL (no RoomGlyph commands emitted today).

- [ ] **Step 3: Emit RoomGlyph commands**

In `src/companion/render.rs`, add a helper and call it from `build_draw_commands` after the Background push (and before pet, so glyphs sit behind the pet):

```rust
/// Scatter a sparse set of biome/dialect room glyphs across the aperture using
/// the SAME selection vocabulary as the watch (room::biome_symbols / biome_style),
/// placed on a deterministic lattice clipped to the circle.
fn push_room_glyph_commands(
    commands: &mut Vec<RoundDrawCommand>,
    scene: &RoundSceneModel,
    layout: &RoundSceneLayout,
) {
    use crate::tui::room::{biome_style, biome_symbols, RoomSpeciesDialect};
    let dialect = RoomSpeciesDialect::for_species(scene.pet.species);
    let symbols = biome_symbols(scene.room.biome.primary, dialect);
    if symbols.is_empty() {
        return;
    }
    let style = biome_style(
        scene.room.biome.primary,
        crate::tui::render_context::ColorCapability::Truecolor,
    );
    let color = match style.fg {
        Some(ratatui::style::Color::Rgb(r, g, b)) => {
            RoundColor(f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 0.55)
        }
        _ => PROP_GLYPH_COLOR,
    };
    let ap = layout.aperture;
    let cell = ap.radius / 5.0; // ~10 glyph slots across the diameter
    if cell <= 0.0 {
        return;
    }
    let mut i = 0usize;
    let steps = 11i32;
    for gy in 0..steps {
        for gx in 0..steps {
            // Sparse: ~1 in 3 lattice points.
            if (gx + gy) % 3 != 0 {
                continue;
            }
            let x = ap.center_x - ap.radius + cell * gx as f32 + cell * 0.5;
            let y = ap.center_y - ap.radius + cell * gy as f32 + cell * 0.5;
            if !ap.contains(x, y) {
                continue;
            }
            // Keep clear of the pet's center disc.
            let dx = x - layout.pet_anchor.x;
            let dy = y - layout.pet_anchor.y;
            if dx * dx + dy * dy < layout.pet_anchor.radius * layout.pet_anchor.radius {
                continue;
            }
            let glyph = symbols[i % symbols.len()];
            i += 1;
            commands.push(RoundDrawCommand {
                kind: RoundDrawKind::RoomGlyph,
                x,
                y,
                radius: cell * 0.5,
                label: Some(glyph),
                text: None,
                spans: Vec::new(),
                color,
            });
        }
    }
}
```

Call it in `build_draw_commands` right after the Background command is pushed (render.rs ~52), before the pet command:

```rust
    push_room_glyph_commands(&mut commands, scene, &layout);
```

(Confirm the exact import path for `ColorCapability` — it is the same type `room::biome_style` takes; match the import already used in `room.rs`. If it lives at `crate::tui::render_context::ColorCapability`, use that; otherwise use the path `room.rs` imports it by.)

- [ ] **Step 4: Implement the live draw arm**

In `src/companion/app.rs`, replace the `RoomGlyph` no-op arm (app.rs:410-413) with:

```rust
        RoundDrawKind::RoomGlyph => {
            if let Some(label) = command.label {
                draw_label(label, command.x, command.y, command.radius, &command.color);
            }
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib companion` then `cargo test`
Expected: PASS. The live path has no automated pixel test (objc draw), so the structural test in Step 1 plus a manual check is the safety net.

- [ ] **Step 6: Commit**

```bash
git add src/companion/render.rs src/companion/app.rs
git commit -m "feat: render companion room glyphs from the shared biome vocabulary"
```

### Task 9: Preview room uses the biome vocabulary

The round preview's `paint_room` uses a dialect-only `#/^/.` lattice. Switch it to the shared `biome_symbols`/`biome_style` so the preview matches the live companion and varies by biome.

**Files:**
- Modify: `src/round/preview.rs` (`paint_room` ~60, `room_symbol` ~172, `palette_color` ~184)

- [ ] **Step 1: Write the failing test**

In `src/round/preview.rs` test module, add:

```rust
    #[test]
    fn preview_room_varies_by_biome() {
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        // Two fixtures with different earned biomes should produce different
        // room glyph sets in the preview frame.
        let vm = WatchViewModel::fixture_with_habitat_props();
        let frame = render_round_preview_frame_from_vm(
            "round-biome",
            "Round Biome",
            &vm,
            datetime!(2026-06-14 12:00 UTC),
            52,
            52,
            RoundRenderCapabilities::preview_truecolor(),
        );
        let room_syms: std::collections::HashSet<_> = frame
            .cells
            .iter()
            .filter(|c| !c.outside_aperture && !c.symbol.trim().is_empty())
            .map(|c| c.symbol.clone())
            .collect();
        assert!(!room_syms.is_empty());
    }
```

(If a second distinct-biome fixture is easy to construct, strengthen this to assert two biomes differ; otherwise this guards that biome-driven symbols render.)

- [ ] **Step 2: Run to verify it fails / passes**

Run: `cargo test --lib round::preview::tests::preview_room_varies_by_biome`
Expected: compiles and runs (may pass trivially). The real change is Step 3 making symbols biome-driven; keep this test as a guard.

- [ ] **Step 3: Rewrite `room_symbol`/`palette_color` to use the biome vocabulary**

In `src/round/preview.rs`, replace `room_symbol` and `palette_color` so they pull from the shared atoms (mirroring Task 8), choosing a symbol deterministically by cell position:

```rust
fn room_symbol_at(scene: &RoundSceneModel, x: u16, y: u16, truecolor: bool) -> (String, String) {
    use crate::tui::room::{biome_style, biome_symbols, RoomSpeciesDialect};
    let dialect = RoomSpeciesDialect::for_species(scene.pet.species);
    let symbols = biome_symbols(scene.room.biome.primary, dialect);
    let glyph = symbols
        .get((x as usize + y as usize) % symbols.len().max(1))
        .copied()
        .unwrap_or('·');
    let style = biome_style(
        scene.room.biome.primary,
        crate::tui::render_context::ColorCapability::Truecolor,
    );
    let fg = match (truecolor, style.fg) {
        (true, Some(ratatui::style::Color::Rgb(r, g, b))) => format!("#{r:02x}{g:02x}{b:02x}"),
        (true, _) => "#808080".to_string(),
        (false, _) => "gray".to_string(),
    };
    (glyph.to_string(), fg)
}
```

Update `paint_room` to call `room_symbol_at(scene, x, y, truecolor)` instead of `room_symbol(scene, truecolor)`. Remove the now-unused `room_symbol`/`palette_color` (clippy will flag). Keep the `(x + y) % 5 == 0` sparsity gate.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib round::preview` then `cargo test --test dev_preview`
Expected: PASS. Round preview frames change (biome glyphs instead of `#/^/.`) — update snapshots via `INSTA_UPDATE=always`, verify the diff is only room glyph/color, stage.

- [ ] **Step 5: Commit**

```bash
git add src/round/preview.rs tests/snapshots/
git commit -m "feat: round preview room uses the shared biome vocabulary"
```

### Task 10: Companion day-phase darkening

Dim the companion background by day-phase (night darker), so the companion shows the same day/night swing as the watch. The asleep/calm dim overlay already exists (app.rs:347-352); add a phase factor to the background color.

**Files:**
- Modify: `src/companion/render.rs` (`biome_background_color` callers / a `phase_dim` factor) and the Background push.

- [ ] **Step 1: Write the failing test**

In `src/companion/render.rs` test module, add:

```rust
    #[test]
    fn night_background_is_darker_than_day() {
        use crate::tui::day::DayPhase;
        use crate::tui::room::RoomBiomeTag;
        let day = phase_dim_background(RoomBiomeTag::Botanical, DayPhase::Day);
        let night = phase_dim_background(RoomBiomeTag::Botanical, DayPhase::Night);
        assert!(night.0 <= day.0 && night.1 <= day.1 && night.2 <= day.2);
        assert!(night != day);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --lib companion::render::tests::night_background_is_darker_than_day`
Expected: FAIL (`phase_dim_background` undefined).

- [ ] **Step 3: Add `phase_dim_background` and use it**

In `src/companion/render.rs`, add:

```rust
/// Biome background scaled down by day-phase so night reads darker.
pub(crate) fn phase_dim_background(
    tag: crate::tui::room::RoomBiomeTag,
    phase: crate::tui::day::DayPhase,
) -> RoundColor {
    use crate::tui::day::DayPhase;
    let base = biome_background_color(tag);
    let k = match phase {
        DayPhase::Day => 1.0,
        DayPhase::Dawn => 0.85,
        DayPhase::Dusk => 0.8,
        DayPhase::Night => 0.6,
    };
    RoundColor(base.0 * k, base.1 * k, base.2 * k, base.3)
}
```

Change the Background command color (from Task 7) to `phase_dim_background(scene.room.biome.primary, scene.room.day_phase)`.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib companion` then `cargo test --test dev_preview`
Expected: PASS. Update round snapshots if bg pinned; verify diff is only bg darkening.

- [ ] **Step 5: Commit**

```bash
git add src/companion/render.rs tests/snapshots/
git commit -m "feat: darken the companion background by day-phase"
```

---

## Task 11: Tranche 2 gate

- [ ] **Step 1: Full verification**

Run: `cargo test`
Run: `cargo fmt --check`
Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: all PASS.

- [ ] **Step 2: Review in color**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```
Confirm with Drew: (a) the four `watch-*` biome rooms read as distinct places (floor + wash + symbols differ); (b) night vs day is a clear swing (sparser, dimmer, cooler — pet included); (c) the round/companion frames show a biome-tinted background and real room glyphs that match the watch vocabulary; (d) the four Default-dialect species' rooms differ.

---

## Definition of done

- A reviewer can tell two different biomes apart from a single watch screenshot (floor, wash, and symbol family all differ).
- The same biome at day vs night reads as two clearly different moods (density, dim, warmth — and the pet retints).
- The macOS companion shows a biome-tinted background and biome/dialect room glyphs drawn from the *same* `biome_symbols`/`biome_style` the watch uses (no more `#/^/.` lattice; no flat constant disc).
- The watch room generator (`room_glyphs_for`) is unchanged and its existing output is preserved except for the deliberate dialect-split / night / floor / wash changes (each covered by an updated snapshot whose diff was verified).

## Testing strategy

- Behavior/structure tests (not exact colors): dialect families distinct; floor palette biome-keyed; wash subtle + biome-distinct; night density clearly lower; pet phase tint cools at night; companion emits in-aperture RoomGlyph commands; companion bg biome-tinted + night-darker; preview room renders biome symbols.
- Snapshot updates (`insta`): every visual change to a watch/round frame updates its `.snap` via `INSTA_UPDATE=always cargo test --test dev_preview`, with the `git diff tests/snapshots/` inspected to confirm the change is only the intended environment edit before staging.
- Color/symbol values are starting points reviewed via `index.html` (Task 11), not pinned in tests.

## Risks and mitigations

- **Snapshot churn across many tasks.** Each task updates only its own frames; always inspect `git diff tests/snapshots/` before staging to confirm scope.
- **`ColorCapability` import path** for `biome_style` from the companion — confirm the exact path `room.rs` uses and mirror it (noted inline in Tasks 8–9).
- **Companion room glyphs crowding the pet.** The pet-disc exclusion in Task 8 keeps the center clear; tune `cell`/sparsity in review.
- **Watch room generator regressions.** We do NOT refactor `room_glyphs_for`; only data (dialect symbols), the floor painter, night scales, the wash pass, and the pet tint change — each snapshot-verified.

## Self-review notes

- **Spec coverage:** shared generator → realized as shared selection atoms (Task 1) reused by companion (Tasks 8–9); biome floor (Task 3); background wash (Task 4); dialect split (Task 2); strengthen night (Task 5); pet ambient light (Task 6); companion biome bg + RoomGlyph + day-phase (Tasks 7, 8, 10). Weather-as-motif deferred per spec.
- **Type consistency:** `biome_symbols(RoomBiomeTag, RoomSpeciesDialect)`, `biome_style(RoomBiomeTag, ColorCapability)`, `biome_floor_palette(RoomBiomeTag)`, `biome_wash_color(RoomBiomeTag)`, `tint_pet_styles_for_phase(&SemanticStyles, DayPhase, f32)`, `biome_background_color(RoomBiomeTag)`, `phase_dim_background(RoomBiomeTag, DayPhase)` used consistently.
- **Deviation from spec is intentional and documented** (no `room_glyphs_for` refactor / byte-identical golden; floor watch-only) — both noted in the header for reviewer awareness.
