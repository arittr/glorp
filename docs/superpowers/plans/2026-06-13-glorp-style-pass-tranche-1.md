# Glorp Style Pass — Tranche 1 (Pets) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every pet a vibrant, species-leaning, per-pet color identity (green eyes preserved) rendered consistently across the watch, the menubar popover, and the macOS companion, behind invariant guardrails and a working in-color review pipeline — then re-sculpt the silhouettes that still collide.

**Architecture:** Build a hand-rolled OKLCH→sRGB resolver (no color crate) and a single `role_color(role, &ResolvedPalette)` function. Converge the four existing role→color sites onto it producing byte-identical output first, then compute a per-pet `ResolvedPalette` (species lean + per-pet jitter, eyes pinned green) once in `build_watch_view_model_at`, carry it on the view model, and feed every surface from it. Fix the preview HTML pipeline so the result is reviewable in color, then iterate the Blob/Ghost silhouette surgery visually against it.

**Tech Stack:** Rust 2021, ratatui 0.29, `unicode-width` (new direct dep). No new color crate — OKLCH→sRGB is hand-rolled.

**Spec:** `docs/superpowers/specs/2026-06-13-glorp-style-pass-design.md` (Tranche 1).

**Scope note:** This plan covers the Tranche 1 *engineering foundation* (Tasks 1–10) as full TDD, plus the *silhouette surgery* (Task 11) as a guided visual-iteration phase — pre-baking exact 11×8 grids would be fake precision that needs rework, so Task 11 specifies targets, guardrails, and the review loop rather than final grids. The ambient (biome/day-phase) lighting layer is deferred to Tranche 2, where biome and the room live. Tranche 2 (shared generator, environments, companion room) is gated on reviewing Tranche 1 and is not planned here.

---

## File Structure

- **Create** `src/pet/palette.rs` — the OKLCH→sRGB resolver, `Rgb`, `ResolvedPalette`, `role_color`, `default_theme_palette`, `resolve_pet_palette`, `species_base_hue`. One responsibility: turn species/traits into resolved per-role RGB. Backend-neutral (returns `Rgb`); each surface adapts.
- **Modify** `src/pet/mod.rs` — add `pub mod palette;`.
- **Modify** `Cargo.toml` — add `unicode-width`.
- **Modify** `src/pet/art.rs` — add display-width + height guard tests.
- **Modify** `src/tui/panels/pet.rs` — `pet_role_style` / `pet_role_spans_for_line` consume a `ResolvedPalette`.
- **Modify** `src/menubar/render.rs` — `role_color_for_profile` sources its base from `palette::role_color`.
- **Modify** `src/companion/app.rs` — `pet_role_color` sources from `palette::role_color`.
- **Modify** `src/round/preview.rs` — `paint_pet_art` honors spans via `role_color`.
- **Modify** `src/tui/view_model.rs` — add `pet_palette: ResolvedPalette` to `WatchViewModel`.
- **Modify** `src/round/model.rs` — add `palette` to `RoundPetModel`, populate from `vm.pet_palette`.
- **Modify** `src/commands/watch.rs` — compute `resolve_pet_palette` in `build_watch_view_model_at`.
- **Modify** `src/dev_preview/frame.rs` + `src/dev_preview/export.rs` — resolve `Color::Indexed`/named to hex so fallback colors survive into HTML.
- **Modify** `src/dev_preview/pets.rs` — render the pet matrix through the per-pet palette; honor `color_capability`.
- **Modify** `src/dev_preview/scenarios.rs` — register a flat-mode pet matrix scenario.
- **Modify** `CLAUDE.md` — declare `art.rs` canonical for templates.

---

## Task 1: Declare `art.rs` canonical for templates (convention, no code)

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Update the convention text**

In `CLAUDE.md`, find the Conventions bullet that reads (near the end of the file):

> The 4 brainstorm/spec directories under `docs/` describe the design intent. `docs/tokenpet/project/pet.jsx` is the source of truth for templates, stage labels (`SPECIES_ARCS`), animation profiles (`SPECIES_ANIM`), and mood eye/mouth overrides — port from there, don't invent.

Replace it with:

> The 4 brainstorm/spec directories under `docs/` describe the design intent. `src/pet/art.rs` is the source of truth for pet templates and silhouettes (the filled-block art has diverged from `pet.jsx` and is not ported back). `docs/tokenpet/project/pet.jsx` remains the reference only for stage labels (`SPECIES_ARCS`), animation profiles (`SPECIES_ANIM`), and mood eye/mouth overrides (`EYES_BY_MOOD`) — port those from there, don't invent.

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: make art.rs canonical for pet templates"
```

---

## Task 2: Display-width + height grid invariants

The current width test counts code points (`art.rs:743` `.chars().count()`), which treats a width-2 (CJK/emoji) glyph as 1. Add a terminal-display-width guard and a height guard before any silhouette surgery. Policy: art uses ambiguous-width-narrow glyphs only; the test uses `unicode-width`'s default `width()` (ambiguous = 1), which matches the current block-element art and catches any always-width-2 glyph.

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/pet/art.rs` (test module, near line 708)

- [ ] **Step 1: Add the dependency**

In `Cargo.toml`, under `[dependencies]`, add (alphabetical-ish, after `toml`):

```toml
unicode-width = "0.2"
```

- [ ] **Step 2: Write the failing tests**

In `src/pet/art.rs`, inside `mod tests` (after `every_template_line_is_eleven_cells_wide`), add:

```rust
    #[test]
    fn every_template_line_is_eleven_display_columns() {
        use unicode_width::UnicodeWidthStr;
        // Terminal columns, not code points. Policy: art uses only glyphs that
        // are width-1 under the ambiguous=narrow assumption (unicode-width's
        // default `width()`); this catches any always-width-2 (CJK/emoji) glyph
        // that `.chars().count()` would miss.
        for species in Species::all() {
            for stage in ALL_STAGES {
                for morph_index in 0..6 {
                    for morph_pup_index in 0..6 {
                        let lines = template_lines(species, stage, morph_index, morph_pup_index);
                        for (row, line) in lines.iter().enumerate() {
                            let rendered = substitute_slots(line);
                            let columns = UnicodeWidthStr::width(rendered.as_str());
                            assert_eq!(
                                columns, 11,
                                "display width != 11 for species={species:?} stage={stage:?} \
                                 morph={morph_index} pup_morph={morph_pup_index} row={row}: \
                                 {rendered:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_template_is_eight_lines() {
        // frame_with_particles overlays exactly 8 art rows (render.rs `.take(8)`);
        // a template with a different line count would silently clip or shift.
        for species in Species::all() {
            for stage in ALL_STAGES {
                for morph_index in 0..6 {
                    for morph_pup_index in 0..6 {
                        let lines = template_lines(species, stage, morph_index, morph_pup_index);
                        assert_eq!(
                            lines.len(),
                            8,
                            "template height != 8 for species={species:?} stage={stage:?} \
                             morph={morph_index} pup_morph={morph_pup_index}"
                        );
                    }
                }
            }
        }
    }
```

- [ ] **Step 3: Run the tests**

Run: `cargo test --lib pet::art::tests::every_template_line_is_eleven_display_columns pet::art::tests::every_template_is_eight_lines`
Expected: PASS. (These are guard tests over existing art. If `eleven_display_columns` FAILS, an existing template uses an always-width-2 glyph — replace it with a width-1 equivalent and re-run; that is a real latent bug this task surfaces.)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock src/pet/art.rs
git commit -m "test: add display-width and height grid invariants for pet art"
```

---

## Task 3: OKLCH→sRGB resolver module

Hand-rolled (no color crate). OKLCH → OKLab → linear sRGB → chroma-reduction gamut map → gamma → `Rgb`.

**Files:**
- Create: `src/pet/palette.rs`
- Modify: `src/pet/mod.rs`

- [ ] **Step 1: Register the module**

In `src/pet/mod.rs`, add alongside the other `pub mod` lines:

```rust
pub mod palette;
```

- [ ] **Step 2: Write the failing tests**

Create `src/pet/palette.rs` with only the tests first:

```rust
//! Pet color resolution: OKLCH -> sRGB, and per-pet/species palettes.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_and_white_are_exact() {
        assert_eq!(oklch_to_rgb(0.0, 0.0, 0.0), Rgb { r: 0, g: 0, b: 0 });
        assert_eq!(oklch_to_rgb(1.0, 0.0, 0.0), Rgb { r: 255, g: 255, b: 255 });
    }

    #[test]
    fn zero_chroma_is_achromatic() {
        let gray = oklch_to_rgb(0.6, 0.0, 210.0);
        assert_eq!(gray.r, gray.g);
        assert_eq!(gray.g, gray.b);
    }

    #[test]
    fn higher_lightness_is_brighter() {
        let dark = oklch_to_rgb(0.3, 0.0, 0.0).r;
        let light = oklch_to_rgb(0.8, 0.0, 0.0).r;
        assert!(light > dark, "{light} !> {dark}");
    }

    #[test]
    fn output_is_always_in_gamut() {
        // Request an absurd chroma; gamut mapping must still yield valid bytes
        // (no panic, no wraparound) by reducing chroma, not clamping channels.
        for hue in (0..360).step_by(15) {
            let _ = oklch_to_rgb(0.7, 0.5, hue as f32);
        }
    }

    #[test]
    fn gamut_mapping_preserves_hue() {
        // An out-of-gamut request and its in-gamut chroma-reduced version
        // should share (approximately) the same hue.
        let requested_hue = 30.0_f32;
        let mapped = oklch_to_rgb(0.7, 0.5, requested_hue);
        let safe = oklch_to_rgb(0.7, 0.08, requested_hue);
        let dh = (rgb_hue(mapped) - rgb_hue(safe)).abs();
        let dh = dh.min(360.0 - dh);
        assert!(dh < 8.0, "hue drift {dh} too large");
    }

    // Test helper: recover OKLCH hue from an Rgb for the hue-preservation test.
    fn rgb_hue(c: Rgb) -> f32 {
        let (a, b) = rgb_to_oklab_ab(c);
        b.atan2(a).to_degrees().rem_euclid(360.0)
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test --lib pet::palette`
Expected: FAIL to compile (`oklch_to_rgb`, `Rgb`, `rgb_to_oklab_ab` undefined).

- [ ] **Step 4: Implement the resolver**

Prepend to `src/pet/palette.rs` (above the test module):

```rust
/// Backend-neutral 8-bit color. Each surface adapts this to its own type
/// (ratatui `Color::Rgb`, the companion `RoundColor`, the menubar `Rgb`, or a
/// `#rrggbb` preview string).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// Convert an OKLCH color (lightness 0..1, chroma >=0, hue in degrees) to sRGB.
/// Out-of-gamut requests are mapped by reducing chroma at fixed lightness and
/// hue (never per-channel clamping, which shifts hue).
pub fn oklch_to_rgb(lightness: f32, chroma: f32, hue_degrees: f32) -> Rgb {
    let in_gamut = |c: f32| oklch_to_linear(lightness, c, hue_degrees).iter().all(|&v| (-1e-4..=1.0 + 1e-4).contains(&v));

    let chroma = if in_gamut(chroma) {
        chroma
    } else {
        // Binary search the largest in-gamut chroma in [0, chroma].
        let mut lo = 0.0_f32;
        let mut hi = chroma;
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if in_gamut(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        lo
    };

    let linear = oklch_to_linear(lightness, chroma, hue_degrees);
    Rgb {
        r: encode_srgb(linear[0]),
        g: encode_srgb(linear[1]),
        b: encode_srgb(linear[2]),
    }
}

fn oklch_to_linear(lightness: f32, chroma: f32, hue_degrees: f32) -> [f32; 3] {
    let h = hue_degrees.to_radians();
    let a = chroma * h.cos();
    let b = chroma * h.sin();

    let l_ = lightness + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = lightness - 0.105_561_35 * a - 0.063_854_17 * b;
    let s_ = lightness - 0.089_484_18 * a - 1.291_485_5 * b;

    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;

    [
        4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s,
        -1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s,
        -0.004_196_086_3 * l - 0.703_418_6 * m + 1.707_614_7 * s,
    ]
}

fn encode_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Recover the OKLab a/b chromatic axes from an sRGB color. Used only by tests
/// and by the (test-supporting) hue checks; kept here so the matrices stay
/// co-located with their inverse.
pub(crate) fn rgb_to_oklab_ab(c: Rgb) -> (f32, f32) {
    let lin = |v: u8| {
        let v = f32::from(v) / 255.0;
        if v <= 0.040_45 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let r = lin(c.r);
    let g = lin(c.g);
    let b = lin(c.b);

    let l = 0.412_165_6 * r + 0.536_275_2 * g + 0.051_457_57 * b;
    let m = 0.211_859_1 * r + 0.680_718_9 * g + 0.107_406_58 * b;
    let s = 0.088_309_795 * r + 0.281_847_8 * g + 0.630_257 * b;

    let l_ = l.cbrt();
    let m_ = m.cbrt();
    let s_ = s.cbrt();

    let a = 1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_;
    let b = 0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_;
    (a, b)
}
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --lib pet::palette`
Expected: PASS (5 tests).

- [ ] **Step 6: Commit**

```bash
git add src/pet/mod.rs src/pet/palette.rs
git commit -m "feat: add hand-rolled OKLCH->sRGB color resolver"
```

---

## Task 4: `ResolvedPalette` + `role_color` + byte-identical default theme

Introduce the shared per-role palette and a single `role_color`. The default palette reproduces the current fixed theme exactly (Body `#efebe4`, Eye `#82bc83`, Mouth `#97918a`, Accent `#f0a646`, Pattern `#504c49`, Particle = Accent), so convergence in Tasks 5–7 is byte-identical.

**Files:**
- Modify: `src/pet/palette.rs`

- [ ] **Step 1: Write the failing test**

In `src/pet/palette.rs` test module, add:

```rust
    #[test]
    fn default_theme_matches_current_fixed_colors() {
        use crate::pet::render::PaletteRoleName::*;
        let p = default_theme_palette();
        assert_eq!(role_color(Body, &p), Rgb::new(0xef, 0xeb, 0xe4));
        assert_eq!(role_color(Eye, &p), Rgb::new(0x82, 0xbc, 0x83));
        assert_eq!(role_color(Mouth, &p), Rgb::new(0x97, 0x91, 0x8a));
        assert_eq!(role_color(Accent, &p), Rgb::new(0xf0, 0xa6, 0x46));
        assert_eq!(role_color(Pattern, &p), Rgb::new(0x50, 0x4c, 0x49));
        assert_eq!(role_color(Particle, &p), Rgb::new(0xf0, 0xa6, 0x46));
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib pet::palette::tests::default_theme_matches_current_fixed_colors`
Expected: FAIL to compile (`ResolvedPalette`, `role_color`, `default_theme_palette` undefined).

- [ ] **Step 3: Implement**

Add to `src/pet/palette.rs` (above tests), and add `use crate::pet::render::PaletteRoleName;` at the top:

```rust
use crate::pet::render::PaletteRoleName;

/// Resolved per-role colors for one pet. `eye` is always the green signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPalette {
    pub body: Rgb,
    pub eye: Rgb,
    pub mouth: Rgb,
    pub accent: Rgb,
    pub pattern: Rgb,
}

pub fn role_color(role: PaletteRoleName, palette: &ResolvedPalette) -> Rgb {
    match role {
        PaletteRoleName::Body => palette.body,
        PaletteRoleName::Eye => palette.eye,
        PaletteRoleName::Mouth => palette.mouth,
        PaletteRoleName::Accent => palette.accent,
        PaletteRoleName::Pattern => palette.pattern,
        PaletteRoleName::Particle => palette.accent,
    }
}

/// The pre-color fixed theme, reproducing `semantic_styles()` pet colors exactly.
pub fn default_theme_palette() -> ResolvedPalette {
    ResolvedPalette {
        body: Rgb::new(0xef, 0xeb, 0xe4),
        eye: Rgb::new(0x82, 0xbc, 0x83),
        mouth: Rgb::new(0x97, 0x91, 0x8a),
        accent: Rgb::new(0xf0, 0xa6, 0x46),
        pattern: Rgb::new(0x50, 0x4c, 0x49),
    }
}
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test --lib pet::palette`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/pet/palette.rs
git commit -m "feat: add ResolvedPalette and role_color with byte-identical default theme"
```

---

## Task 5: Converge the watch onto `role_color` (byte-identical)

Make `pet_role_style` build its `Style` from a `ResolvedPalette` instead of `SemanticStyles`, preserving the eye's BOLD. Thread a palette through `pet_role_spans_for_line`. Callers pass `default_theme_palette()` in this task (byte-identical); Task 9 swaps in the per-pet palette.

**Files:**
- Modify: `src/tui/panels/pet.rs` (`pet_role_style` ~1658, `pet_role_spans_for_line` ~1584, and their callers within the panel render path)

- [ ] **Step 1: Write the failing test**

In `src/tui/panels/pet.rs` test module, add:

```rust
    #[test]
    fn pet_role_style_uses_resolved_palette_with_bold_eye() {
        use crate::pet::palette::{default_theme_palette, Rgb};
        use crate::pet::render::PaletteRoleName;
        let p = default_theme_palette();
        let eye = pet_role_style(PaletteRoleName::Eye, &p);
        assert_eq!(eye.fg, Some(ratatui::style::Color::Rgb(0x82, 0xbc, 0x83)));
        assert!(eye.add_modifier.contains(ratatui::style::Modifier::BOLD));
        let body = pet_role_style(PaletteRoleName::Body, &p);
        assert_eq!(body.fg, Some(ratatui::style::Color::Rgb(0xef, 0xeb, 0xe4)));
        assert!(!body.add_modifier.contains(ratatui::style::Modifier::BOLD));
        let _ = Rgb::new(0, 0, 0); // keep import used
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib tui::panels::pet::tests::pet_role_style_uses_resolved_palette_with_bold_eye`
Expected: FAIL to compile (`pet_role_style` still takes `&SemanticStyles`).

- [ ] **Step 3: Reimplement `pet_role_style`**

Replace the body of `pet_role_style` (`src/tui/panels/pet.rs:1658`) with:

```rust
pub(crate) fn pet_role_style(
    role: PaletteRoleName,
    palette: &crate::pet::palette::ResolvedPalette,
) -> Style {
    let rgb = crate::pet::palette::role_color(role, palette);
    let mut style = Style::default().fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    if matches!(role, PaletteRoleName::Eye) {
        style = style.add_modifier(Modifier::BOLD);
    }
    style
}
```

Ensure `Color` and `Modifier` are imported in this file (they are used elsewhere in `pet.rs`; if not in scope at this location, add `use ratatui::style::{Color, Modifier};`).

- [ ] **Step 4: Thread the palette through `pet_role_spans_for_line`**

Change `pet_role_spans_for_line`'s signature (`src/tui/panels/pet.rs:1584`) from taking `styles: &'a SemanticStyles` to taking BOTH the styles (still needed for the `styles.pet_body` default fill) and the palette. Minimal change: add a parameter and replace the body-default and the `pet_role_style` call:

- Signature: add `palette: &'a crate::pet::palette::ResolvedPalette,` after `styles`.
- Replace `return vec![Span::styled(art_line, styles.pet_body)];` and the two other `styles.pet_body` fills with `pet_role_style(PaletteRoleName::Body, palette)`.
- Replace `let style = pet_role_style(segment.role, styles);` with `let style = pet_role_style(segment.role, palette);`.

- [ ] **Step 5: Update callers within the panel**

Find the call sites of `pet_role_spans_for_line` in `src/tui/panels/pet.rs` (the panel render path). At each, obtain the palette. In this task pass the default:

```rust
let palette = crate::pet::palette::default_theme_palette();
```
and pass `&palette` as the new argument. (Task 9 replaces this with the view-model palette.)

- [ ] **Step 6: Run tests**

Run: `cargo test --lib tui::panels::pet`
Expected: PASS, including the new test and all existing pet-panel tests (byte-identical).

Run the broader watch/preview snapshot tests too:
Run: `cargo test`
Expected: PASS (no snapshot drift — output is byte-identical).

- [ ] **Step 7: Commit**

```bash
git add src/tui/panels/pet.rs
git commit -m "refactor: route watch pet styling through role_color (byte-identical)"
```

---

## Task 6: Converge the menubar onto `role_color` (byte-identical)

`role_color_for_profile` keeps its source-accent and sleep-dim logic but sources the base role color from `palette::role_color(role, &default_theme_palette())`. The local `COLOR_*` constants used only by `role_color` are removed; constants used elsewhere in the file (e.g. `COLOR_FG`, `COLOR_DIM`, `COLOR_ACCENT` in `StyledRun` builders) stay.

**Files:**
- Modify: `src/menubar/render.rs`

- [ ] **Step 1: Write the failing test**

In `src/menubar/render.rs` test module, add:

```rust
    #[test]
    fn menubar_role_base_matches_resolved_palette() {
        use crate::pet::palette::{default_theme_palette, role_color};
        use crate::pet::render::PaletteRoleName::*;
        let p = default_theme_palette();
        for role in [Body, Eye, Mouth, Pattern] {
            let rgb = role_color(role, &p);
            assert_eq!(role_color_base(role), Rgb(rgb.r, rgb.g, rgb.b));
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib menubar::render::tests::menubar_role_base_matches_resolved_palette`
Expected: FAIL to compile (`role_color_base` undefined).

- [ ] **Step 3: Implement**

Replace `fn role_color(role: PaletteRoleName) -> Rgb` (`src/menubar/render.rs:67`) with a delegating `role_color_base`:

```rust
fn role_color_base(role: PaletteRoleName) -> Rgb {
    let rgb = crate::pet::palette::role_color(role, &crate::pet::palette::default_theme_palette());
    Rgb(rgb.r, rgb.g, rgb.b)
}
```

In `role_color_for_profile` (`src/menubar/render.rs:81`), change the first line `let base = role_color(role);` to `let base = role_color_base(role);`. Remove any now-unused `COLOR_*` consts (compiler will warn — delete the dead ones; keep those still referenced by `StyledRun` builders).

- [ ] **Step 4: Run tests**

Run: `cargo test --lib menubar::render`
Expected: PASS.
Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS (no dead-code warnings for removed consts).

- [ ] **Step 5: Commit**

```bash
git add src/menubar/render.rs
git commit -m "refactor: source menubar pet colors from role_color (byte-identical)"
```

---

## Task 7: Converge the companion-live onto `role_color` (byte-identical)

`pet_role_color` sources from `palette::role_color` instead of `semantic_styles()`, then wraps in `RoundColor`.

**Files:**
- Modify: `src/companion/app.rs` (`pet_role_color` ~503)

- [ ] **Step 1: Write the failing test**

In `src/companion/app.rs` test module, add:

```rust
    #[test]
    fn companion_pet_role_color_matches_resolved_palette() {
        use crate::pet::palette::{default_theme_palette, role_color};
        use crate::pet::render::PaletteRoleName::*;
        let p = default_theme_palette();
        for role in [Body, Eye, Mouth, Accent, Pattern, Particle] {
            let rgb = role_color(role, &p);
            assert_eq!(pet_role_color(role), Some(rgb_color(rgb.r, rgb.g, rgb.b)));
        }
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib companion::app::tests::companion_pet_role_color_matches_resolved_palette`
Expected: FAIL (current `pet_role_color` maps `Particle` via `pet_accent` too, but routes through `semantic_styles()` — the test pins the resolver path).

- [ ] **Step 3: Implement**

Replace `pet_role_color` (`src/companion/app.rs:503`) with:

```rust
fn pet_role_color(role: PaletteRoleName) -> Option<RoundColor> {
    let rgb = crate::pet::palette::role_color(role, &crate::pet::palette::default_theme_palette());
    Some(rgb_color(rgb.r, rgb.g, rgb.b))
}
```

`style_color` becomes unused for pet roles; if the compiler flags it as dead, leave it only if still referenced, else remove it.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib companion::app`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/companion/app.rs
git commit -m "refactor: source companion pet colors from role_color (byte-identical)"
```

---

## Task 8: Fix the companion preview to honor pet spans

`round/preview.rs::paint_pet_art` hardcodes cream and ignores `art_spans`. Color it per role via `role_color`. This *changes* the companion preview output (a bug fix), so it gets a positive test.

**Files:**
- Modify: `src/round/preview.rs` (`paint_pet_art` ~82; `RoundSceneModel.pet.art_spans` is already available)

- [ ] **Step 1: Write the failing test**

In `src/round/preview.rs` test module, add (mirroring the existing test setup that builds a scene + layout):

```rust
    #[test]
    fn preview_pet_colors_eye_and_body_differently() {
        use crate::tui::view_model::WatchViewModel;
        use time::macros::datetime;
        let vm = WatchViewModel::fixture_with_habitat_props();
        let frame = render_round_preview_frame_from_vm(
            "round-color",
            "Round Color",
            &vm,
            datetime!(2026-06-13 18:00 UTC),
            52,
            52,
            RoundRenderCapabilities::preview_truecolor(),
        );
        let fgs: std::collections::HashSet<_> = frame
            .cells
            .iter()
            .filter(|c| !c.symbol.trim().is_empty())
            .filter_map(|c| c.fg.clone())
            .collect();
        // More than one distinct pet/room fg means spans are honored (not flat cream).
        assert!(fgs.len() > 1, "expected multiple fg colors, got {fgs:?}");
    }
```

(Adjust the imports/fixture/`RoundRenderCapabilities` constructor to match the existing tests already in this file.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib round::preview::tests::preview_pet_colors_eye_and_body_differently`
Expected: FAIL (pet art is flat `#efebe4`; only the room glyphs add a second color — assertion may already pass by accident, so make it stronger if needed by asserting an eye-green cell exists; see Step 3).

- [ ] **Step 3: Implement**

In `paint_pet_art` (`src/round/preview.rs:82`), replace the flat-color block. For each non-space char, look up its role from `scene.pet.art_spans` (reuse the same role-lookup approach as `companion/app.rs::role_for_pet_cell` — add a local helper or import it) and color via `role_color`:

```rust
    for (row, line) in scene.pet.art_lines.iter().enumerate() {
        let mut col = 0i32;
        for (char_index, ch) in line.chars().enumerate() {
            let display_width = Line::from(ch.to_string()).width() as i32;
            if ch != ' ' {
                let role = role_for_pet_cell(&scene.pet.art_spans, row, char_index);
                let rgb = crate::pet::palette::role_color(
                    role,
                    &crate::pet::palette::default_theme_palette(),
                );
                let fg = if truecolor {
                    format!("#{:02x}{:02x}{:02x}", rgb.r, rgb.g, rgb.b)
                } else {
                    flat_role_name(role).to_string()
                };
                set_cell(cells, width, start_x + col, start_y + row as i32, ch.to_string(), Some(fg));
            }
            col += display_width;
        }
    }
```

Add a `role_for_pet_cell` helper in this file (copy the 6-line body from `companion/app.rs:495`) and a `flat_role_name(role) -> &'static str` returning sensible ANSI names (`Eye => "green"`, `Accent | Particle => "yellow"`, else `"white"`). Replace the per-cell role-aware fg.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib round::preview`
Expected: PASS (including existing round-preview tests — only pet fg changes).

- [ ] **Step 5: Commit**

```bash
git add src/round/preview.rs
git commit -m "fix: color companion preview pet by role instead of flat cream"
```

---

## Task 9: Turn on color — per-pet species-leaning palette

Compute a per-pet `ResolvedPalette` once and carry it everywhere. Eyes stay green; body/mouth/accent/pattern lean by species and vary by seed.

**Files:**
- Modify: `src/pet/palette.rs` (`resolve_pet_palette`, `species_base_hue`)
- Modify: `src/tui/view_model.rs` (add field)
- Modify: `src/commands/watch.rs` (compute + populate)
- Modify: `src/round/model.rs` (add field, populate from vm)
- Modify: `src/tui/panels/pet.rs`, `src/menubar/render.rs`, `src/companion/app.rs`, `src/round/preview.rs` (consume the real palette instead of `default_theme_palette()`)

- [ ] **Step 1: Write the failing tests (resolver)**

In `src/pet/palette.rs` test module, add:

```rust
    fn traits_with_hue(hue: u16) -> crate::pet::generation::VisibleTraits {
        crate::pet::generation::VisibleTraits {
            eyes: "o o".into(), mouth: "w".into(), pattern: "...".into(), accent: "*".into(),
            palette_index: 0, morph_index: 0, morph_pup_index: 0,
            seed_hue: hue, saturation_percent: 90,
        }
    }

    #[test]
    fn resolve_is_deterministic() {
        use crate::pet::generation::Species;
        let a = resolve_pet_palette(Species::Fuzz, &traits_with_hue(42));
        let b = resolve_pet_palette(Species::Fuzz, &traits_with_hue(42));
        assert_eq!(a, b);
    }

    #[test]
    fn eyes_are_green_for_every_species() {
        use crate::pet::generation::Species;
        let green = resolve_pet_palette(Species::Fuzz, &traits_with_hue(0)).eye;
        for s in Species::all() {
            let p = resolve_pet_palette(s, &traits_with_hue(123));
            assert_eq!(p.eye, green, "eye drifted for {s:?}");
        }
        assert!(green.g > green.r && green.g > green.b, "eye not green: {green:?}");
    }

    #[test]
    fn species_lean_separates_bodies() {
        use crate::pet::generation::Species;
        let fuzz = resolve_pet_palette(Species::Fuzz, &traits_with_hue(0)).body;
        let blob = resolve_pet_palette(Species::Blob, &traits_with_hue(0)).body;
        assert_ne!(fuzz, blob);
    }

    #[test]
    fn per_pet_variety_within_species() {
        use crate::pet::generation::Species;
        let a = resolve_pet_palette(Species::Fuzz, &traits_with_hue(10)).body;
        let b = resolve_pet_palette(Species::Fuzz, &traits_with_hue(300)).body;
        assert_ne!(a, b);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib pet::palette`
Expected: FAIL to compile (`resolve_pet_palette` undefined).

- [ ] **Step 3: Implement the per-pet resolver**

Add to `src/pet/palette.rs`. The starting hue/chroma values are concrete but **tunable via `index.html`** (Task 10); they are not placeholders.

```rust
use crate::pet::generation::{Species, VisibleTraits};

/// Hue (OKLCH degrees) each species leans toward.
fn species_base_hue(species: Species) -> f32 {
    match species {
        Species::Fuzz => 70.0,     // warm amber
        Species::Blob => 195.0,    // teal
        Species::Ghost => 300.0,   // violet
        Species::Glitch => 135.0,  // acid green
        Species::Crystal => 230.0, // ice blue
        Species::Mech => 250.0,    // steel
    }
}

/// Pinned green eye signature (same for every species).
const EYE_HUE: f32 = 142.0;

pub fn resolve_pet_palette(species: Species, traits: &VisibleTraits) -> ResolvedPalette {
    let base = species_base_hue(species);
    // Per-pet hue jitter: map seed_hue (0..360) to +-18 degrees off the family.
    let jitter = (f32::from(traits.seed_hue) / 360.0 - 0.5) * 36.0;
    let h = (base + jitter).rem_euclid(360.0);
    let sat = f32::from(traits.saturation_percent) / 100.0; // 0.82..1.0

    let role = |lightness: f32, chroma: f32, hue: f32| oklch_to_rgb(lightness, chroma * sat, hue);

    ResolvedPalette {
        body: role(0.80, 0.11, h),
        eye: oklch_to_rgb(0.84, 0.15, EYE_HUE),
        mouth: role(0.78, 0.09, h + 30.0),
        accent: role(0.80, 0.17, h + 90.0),
        pattern: role(0.70, 0.07, h + 150.0),
    }
}
```

- [ ] **Step 4: Run resolver tests**

Run: `cargo test --lib pet::palette`
Expected: PASS.

- [ ] **Step 5: Carry the palette on the view model**

In `src/tui/view_model.rs`, add to `WatchViewModel` (after `pet_render`):

```rust
    pub pet_palette: crate::pet::palette::ResolvedPalette,
```

Update any `WatchViewModel` test fixtures in this file (e.g. `fixture_with_habitat_props`) to set `pet_palette: crate::pet::palette::default_theme_palette(),` (fixtures stay on the default so existing snapshot expectations are explicit; only the production builder computes per-pet).

- [ ] **Step 6: Populate it in the builder**

In `src/commands/watch.rs`, inside `build_watch_view_model_at`, before the `Ok(WatchViewModel { ... })` literal, add:

```rust
    let pet_palette = crate::pet::palette::resolve_pet_palette(species, &state.pet.traits);
```

and add `pet_palette,` to the struct literal (next to `pet_render`). (`species` is already in scope; `state.pet.traits` is the `VisibleTraits`.)

- [ ] **Step 7: Carry it to the companion**

In `src/round/model.rs`, add to `RoundPetModel`:

```rust
    pub palette: crate::pet::palette::ResolvedPalette,
```

and in `derive_round_scene_model`, set `palette: vm.pet_palette,` in the `RoundPetModel { ... }` literal.

- [ ] **Step 8: Consume the real palette at all four sites**

Replace `default_theme_palette()` with the carried palette:
- `src/tui/panels/pet.rs`: at the `pet_role_spans_for_line` call sites, use `&vm.pet_palette` (the panel has the view model) instead of the local default.
- `src/menubar/render.rs`: change `role_color_base(role)` to take and use `vm.pet_palette` — i.e. inline `crate::pet::palette::role_color(role, &vm.pet_palette)` inside `role_color_for_profile`, dropping the separate `role_color_base` (or give it a `&ResolvedPalette` param).
- `src/companion/app.rs`: `pet_role_color` needs the palette — thread `scene.pet.palette` from the call in `draw_pet_art_block` (pass it down as a parameter) instead of `default_theme_palette()`.
- `src/round/preview.rs`: use `scene.pet.palette` instead of `default_theme_palette()`.

- [ ] **Step 9: Run the full suite**

Run: `cargo test`
Expected: PASS. Snapshot tests that used a default-palette fixture stay stable; the production path now emits per-pet color. If a snapshot test built its fixture via the real builder, update its expectation deliberately (color is now on).

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: PASS.

- [ ] **Step 10: Commit**

```bash
git add src/pet/palette.rs src/tui/view_model.rs src/commands/watch.rs src/round/model.rs src/tui/panels/pet.rs src/menubar/render.rs src/companion/app.rs src/round/preview.rs
git commit -m "feat: vibrant per-pet species-leaning color (green eyes preserved)"
```

---

## Task 10: Fix the in-color review pipeline + flat-mode pet scenario

`color_to_css` emits `ansi-{index}` for `Color::Indexed`, which `render_cell_html`'s `is_hex_color` filter drops, so the fallback palette is invisible in `index.html`. And `dev_preview/pets.rs` ignores `color_capability` and renders the dead theme. Fix both so the review surface shows what ships.

**Files:**
- Modify: `src/dev_preview/frame.rs` (`color_to_css` resolves Indexed/named to hex)
- Modify: `src/dev_preview/pets.rs` (render through the per-pet palette; honor capability)
- Modify: `src/dev_preview/scenarios.rs` (register a flat-mode pet matrix)

- [ ] **Step 1: Write the failing test (Indexed → hex)**

In `src/dev_preview/frame.rs` test module, add:

```rust
    #[test]
    fn indexed_color_resolves_to_hex() {
        let css = color_to_css(Some(Color::Indexed(42)));
        assert!(
            css.as_deref().map(|c| c.starts_with('#') && c.len() == 7).unwrap_or(false),
            "indexed color did not resolve to hex: {css:?}"
        );
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --lib dev_preview::frame::tests::indexed_color_resolves_to_hex`
Expected: FAIL (`color_to_css` returns `ansi-42`).

- [ ] **Step 3: Implement Indexed → hex**

In `src/dev_preview/frame.rs`, replace the `Color::Indexed(index)` arm of `color_to_css` with a resolution to the standard xterm-256 hex. Add a helper `fn ansi_256_to_rgb(index: u8) -> (u8, u8, u8)` implementing the standard cube (0–15 system colors table, 16–231 6×6×6 cube, 232–255 grayscale ramp) and format it:

```rust
        Color::Indexed(index) => {
            let (r, g, b) = ansi_256_to_rgb(index);
            Some(format!("#{r:02x}{g:02x}{b:02x}"))
        }
```

```rust
fn ansi_256_to_rgb(index: u8) -> (u8, u8, u8) {
    const SYSTEM: [(u8, u8, u8); 16] = [
        (0x00, 0x00, 0x00), (0x80, 0x00, 0x00), (0x00, 0x80, 0x00), (0x80, 0x80, 0x00),
        (0x00, 0x00, 0x80), (0x80, 0x00, 0x80), (0x00, 0x80, 0x80), (0xc0, 0xc0, 0xc0),
        (0x80, 0x80, 0x80), (0xff, 0x00, 0x00), (0x00, 0xff, 0x00), (0xff, 0xff, 0x00),
        (0x00, 0x00, 0xff), (0xff, 0x00, 0xff), (0x00, 0xff, 0xff), (0xff, 0xff, 0xff),
    ];
    match index {
        0..=15 => SYSTEM[index as usize],
        16..=231 => {
            let i = index - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            (steps[(i / 36) as usize], steps[((i / 6) % 6) as usize], steps[(i % 6) as usize])
        }
        232..=255 => {
            let v = 8 + (index - 232) * 10;
            (v, v, v)
        }
    }
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test --lib dev_preview::frame`
Expected: PASS.

- [ ] **Step 5: Render the pet matrix through the per-pet palette**

In `src/dev_preview/pets.rs::render_pet_cell`, replace the `styles`-driven span coloring with the per-pet palette. Build the palette from the generated pet and pass it to `pet_role_spans_for_line`:

```rust
    let pet = generate_pet(&format!("glorp-preview-{}", species.as_str())).with_species(species);
    let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
    // ... render_pet(...) as before ...
    for (line_index, art_line) in rendered.lines.iter().enumerate() {
        lines.push(Line::from(pet_role_spans_for_line(
            art_line, line_index, &rendered.spans, &styles, &palette, None,
        )));
    }
```

(The `styles` arg remains for the label line; the new `&palette` arg is the one added in Task 5.)

- [ ] **Step 6: Add a flat-mode pet matrix scenario**

In `src/dev_preview/pets.rs`, factor `render_pet_matrix` to take an id/title and a `ColorCapability`, threading capability into how colors resolve (Truecolor → `Color::Rgb`; Flat → nearest ANSI named/indexed). Emit two frames from `pet_frames`:

```rust
pub fn pet_frames(ctx: &PreviewRenderContext) -> Result<Vec<PreviewFrame>> {
    Ok(vec![
        render_pet_matrix(ctx, "pet-species-stage", "Pet Species Stage", ColorCapability::Truecolor),
        render_pet_matrix(ctx, "pet-species-stage-flat", "Pet Species Stage (Flat)", ColorCapability::Flat),
    ])
}
```

For Flat, map each resolved `Rgb` to the nearest ANSI color via a small helper (or reuse an existing capability-aware path if one exists in `RenderContext`). Keep the truecolor frame's `id` unchanged so existing metadata/tests still match.

- [ ] **Step 7: Register the new scenario id**

In `src/dev_preview/scenarios.rs`:
- Add a `scenario_metadata` arm for `"pet-species-stage-flat"` (copy the `"pet-species-stage"` arm, change the description to note flat/non-truecolor, set `color_capability` to flat).
- Add `"pet-species-stage-flat"` to the hardcoded id list in `scenarios::tests::all_selection_writes_watch_and_pet_scenarios` (the test enumerates every expected id in order).

- [ ] **Step 8: Run the suite + regenerate the preview**

Run: `cargo test`
Expected: PASS (including `dev_preview` integration tests and the scenario id list).

Regenerate and eyeball in color:
```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```
Expected: the pet matrix now shows vibrant, species-distinct, per-pet color with green eyes; the flat matrix shows the fallback still distinguishes species.

- [ ] **Step 9: Commit**

```bash
git add src/dev_preview/frame.rs src/dev_preview/pets.rs src/dev_preview/scenarios.rs
git commit -m "fix: render preview in real color; add flat-mode pet matrix"
```

---

## Task 11: Silhouette surgery (guided visual iteration)

Now that color is on and reviewable, re-sculpt the silhouettes that still collide. This is craft: each grid is designed in the loop, validated by the Task 2 invariants, and reviewed in color via `index.html`. Do **not** treat the target descriptions as final glyphs — iterate.

**Targets (from the spec):**
- **Blob — make it melt.** Asymmetric body: off-center cap, drip-tongues of different lengths left vs. right, a sag on one side; morphs deform rather than re-garnish. Keep round `( )` walls. Shading: light `░` cap → dark `▒▓` belly + a `°` specular dot.
- **Ghost — cloth, not a capsule.** Drop the `|█...█|` pipe-walls. Billowing sheet: tapering top, scalloped `‿‿`/`⌇` hem, fade-to-nothing bottom (every line still space-padded to 11). Shading: vertical fade `█→▒→░→ `.
- **Fuzz — light touch.** Bring the tail forward to S3 (it currently appears only at S5+); add vertical `░▒` column shading for fur grain. No silhouette rebuild.
- **All six — propagate Crystal's directional shading** and confirm particle signatures read.

**Files:**
- Modify: `src/pet/art.rs` (the `BLOB_*`, `GHOST_*`, `FUZZ_*` template constants)

**Per-species loop (repeat for Blob, then Ghost, then Fuzz):**

- [ ] **Step 1: Edit the templates** for one species/stage/morph in `src/pet/art.rs`, keeping every line exactly 11 columns and 8 lines, and slot markers (`{eyes}` 3, `{mouth}` 1, `{pattern}` 3, `{accent}` 1) where roles belong.

- [ ] **Step 2: Run the invariants**

Run: `cargo test --lib pet::art`
Expected: PASS (`every_template_line_is_eleven_display_columns`, `every_template_is_eight_lines`, and the existing code-point test all green).

- [ ] **Step 3: Review in color**

```bash
cargo run -- dev-preview --scenario pets --out target/glorp-preview
open target/glorp-preview/index.html
```
Inspect the species across all seven stages in both the truecolor and flat matrices. Iterate Steps 1–3 until the silhouette reads.

- [ ] **Step 4: Commit** (one commit per species)

```bash
git add src/pet/art.rs
git commit -m "feat: re-sculpt <species> silhouette"
```

---

## Task 12: Tranche 1 gate

- [ ] **Step 1: Full verification**

Run: `cargo test`
Run: `cargo fmt --check`
Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: all PASS.

- [ ] **Step 2: Definition-of-done review (in color)**

```bash
cargo run -- dev-preview --scenario all --out target/glorp-preview
open target/glorp-preview/index.html
```
Confirm with Drew: (a) all six species nameable blind from one screenshot; (b) two seeds of one species read as the same species yet individually distinct; (c) the watch, menubar popover, and companion render the same pet consistently; (d) the flat matrix keeps species distinguishable.

- [ ] **Step 3: Decide on Tranche 2** (shared generator + environments + companion room) based on whether environments still feel too similar now that pet color and silhouettes have landed.

---

## Self-review notes

- **Spec coverage:** color resolver (T3–T4, T9), four-site convergence incl. menubar (T5–T7), companion preview span fix (T8), green-eye signature (T9), species-lean + per-pet variety (T9), display-width/height invariants (T2), color-review pipeline + flat fallback (T10), pet.jsx canonical (T1), definition of done (T12), Blob/Ghost surgery + Fuzz tail (T11). Ambient lighting and the shared generator/environments/companion-room are explicitly Tranche 2 (gated), per the spec's build order.
- **Type consistency:** `Rgb { r, g, b }`, `ResolvedPalette { body, eye, mouth, accent, pattern }`, `role_color(PaletteRoleName, &ResolvedPalette) -> Rgb`, `resolve_pet_palette(Species, &VisibleTraits) -> ResolvedPalette`, `oklch_to_rgb(f32, f32, f32) -> Rgb` are used consistently across tasks. `pet_role_style` / `pet_role_spans_for_line` gain a `&ResolvedPalette` param used identically by every caller.
- **Known starting-value note:** the species hues/chromas in T9 and ANSI-flat mappings in T10 are concrete but tuned in the T10/T11 review loop — not placeholders.
