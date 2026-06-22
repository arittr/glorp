# Phase 3 — Color & Eyes-Mood Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax.

**Goal:** Make each species' body color read as a distinct identity, give particles their own hue, and make the resting eye color encode mood (green at rest → warm excited → blue tired → grey wilted) at animation-tick cadence while holding a ≥3:1 eye/body luminance contrast floor.

**Architecture:** Pet color is OKLCH-resolved per pet in `src/pet/palette.rs::resolve_pet_palette` from `(species, seed)` and carried in `WatchViewModel::pet_palette`, which every live surface (watch, menubar, companion) reads through `role_color`. The ~10s worker rebuild of `pet_palette` stays mood-blind; a new per-tick step in `rerender_pet_for_view_model` mutates `vm.pet_palette.eye` from `vm.pet_render.mood` so eye color rides mood without lagging the eye glyph that `expression_for` already updates per tick. Body chroma/hue retune and the new particle hue live entirely inside `resolve_pet_palette`.

**Tech Stack:** Rust, ratatui, SQLite; tests via cargo + assert_cmd.

## Global Constraints

- Templates are exactly 11 display columns × 8 lines; the `Template` type alias is `[&'static str; 8]` (`src/pet/art.rs`).
- Every art glyph (and every `{slot}` filler) is width-1 under unicode-width's default (ambiguous=narrow); enforced by `every_template_line_is_eleven_display_columns` (`src/pet/art.rs`).
- Eye/accent glyphs must be East-Asian-Width Neutral or Narrow, never Ambiguous; `◇◆◈●○` are Ambiguous (kept only per the Crystal decision — non-blocking lint).
- Growth cell bands (occupied non-space cells across the 8 art rows, fixed reference state): S0:1-4 · S1:5-10 · S2:11-20 · S3:21-34 · S4:35-50 · S5:51-66 · S6:67-88 — disjoint, strictly increasing, S4<S5<S6.
- S6 fills all 8 art rows; the sparkle no longer overwrites art rows 0/7 (asserted as a separate structural check, not in the size count).
- Color is truecolor-first, two tiers only: `ColorCapability::{Truecolor, Flat}` (`src/tui/style.rs:54`); honor `NO_COLOR`/`TERM=dumb`; under Flat pets render monochrome carried by silhouette; sub-truecolor is ratatui's automatic downgrade, not engineered here.
- Tamagotchi spirit: calm over flashy, night calmer than day, nurturing companion not optimizer; no death — floor state is `Mood::Wilted`.
- Only real signals drive content (growth/mood/biome/props/scene-moments trace to observed token usage + clock); the immature-pet zero-feast invariant is preserved (`flat_and_immature_pets_render_zero_motes`, `src/tui/panels/pet/ambient.rs:798`).
- The renderer stays content-agnostic: species/stage character lives in `art.rs` templates + palette, never in renderer special-casing.
- `cargo clippy --all-targets --all-features -- -D warnings` must stay clean; test-only helper fns must be `#[cfg(test)]`.
- Test output must be pristine; intentional error output must be captured and asserted.
- Test isolation: integration tests use `tempfile::tempdir()` + `GLORP_CONFIG_DIR`; when testing helper failures, pin BOTH `GLORP_CCUSAGE_BIN` and `GLORP_CCUSAGE_CODEX_BIN`.
- Commit frequently (do not ask first); WIP branch off `main`; never `git add -A` without a prior `git status`.
- Identity data is never touched: no `state.json` schema change; `seed`/`accepted_name`/`xp`/vitals/stage/calibration/seen-transitions untouched. A one-time visual reset is accepted.
- Do NOT call `apply_usage_poll` from production code (`#[doc(hidden)]` test wrapper).

---

## Phase boundary & what is already in place when Phase 3 starts

**Consumes from Phases 1–2 (do not build these — they exist):**
- `stage_template_lines(species, stage, seed) -> [String; 8]` (`src/pet/art.rs`) and the role/slot structure (`{eyes}` 3-cell, `{mouth}` 1-cell, `{pattern}`, `{accent}`).
- The 42 base silhouettes + standardized mood faces in `expression_for` (`src/pet/render.rs`).
- The invariant test helpers (`rendered_occupied_cells`, `assert_in_stage_band`, etc.) in `art.rs`.

**Produces for Phase 4 (Liveliness will consume these exact names):**
- `PaletteRoleName::Particle` keeps its own resolved color (`palette.particle`, new field) — Phase 4's `Corruption` role joins the same `role_color`/`colors.rs`/`style.rs` plumbing this phase touches.
- `eye_color_for_mood(mood: Mood) -> Rgb` and `apply_mood_eye_color(palette: &mut ResolvedPalette, mood: Mood)` (`src/pet/palette.rs`) — Phase 4's corruption "never touches eye-center" rule relies on the eye color path being a single hook in `rerender_pet_for_view_model`.

**Produces for Phase 5 (Habitat) — none directly.** Phase 5 consumes Phase 1's feet helpers, not Phase 3 color.

**Cross-phase interface signatures this plan must use verbatim (from CONTRACT §2.6 / §2.7):**
```rust
// src/pet/palette.rs
fn species_base_hue(species: Species) -> f32;     // retuned to identity hues
fn species_body_chroma(species: Species) -> f32;  // NEW knob, raised off pinned ~0.10
pub fn eye_color_for_mood(mood: Mood) -> Rgb;      // mood -> eye color
pub fn apply_mood_eye_color(palette: &mut ResolvedPalette, mood: Mood);
```

---

## File Structure

| File | Create/Modify | Responsibility in Phase 3 |
|---|---|---|
| `src/pet/palette.rs` | Modify | Raise body chroma (`species_body_chroma`), retune `species_base_hue`, add `particle: Rgb` to `ResolvedPalette` + its own hue, un-pin `EYE_HUE` via `eye_color_for_mood`/`apply_mood_eye_color`, per-species resting-eye lightness for green-colliding species, relative-luminance contrast helper + tests. |
| `src/pet/render.rs` | Modify | Delete the dead `palette_roles` fn + `PaletteRoles`/`PaletteRole` types + `role`/`with_saturation` helpers (zero production callers; `EYE_HUE`'s duplicate). |
| `src/pet/palette.rs` `role_color` | Modify | `Particle => palette.particle` (was `palette.accent`). |
| `src/tui/panels/pet/colors.rs` | Modify | `seed_pet_palette` / `palette_from_styles` carry the new `particle` field; re-validate the `saturating_add` lift chain at higher chroma. |
| `src/commands/watch.rs` `rerender_pet_for_view_model` | Modify | Per-tick hook: `apply_mood_eye_color(&mut vm.pet_palette, vm.pet_render.mood)` before re-render. |
| `src/dev_preview/pets.rs` | Modify | Apply `apply_mood_eye_color` per cell so the preview mood set (Phase 2 added it) shows correct eye colors. |
| `tests/generation.rs` | Modify | Remove the dead-code-only `palette_roles_follow_tokenpet_hue_offsets` test. |

---

## Task 1 — Add a relative-luminance contrast helper (the measuring stick)

Everything in this phase is judged against the ≥3:1 eye/body contrast floor. Build the ruler first, as a `#[cfg(test)]` helper, before changing any color.

**Files:**
- Modify: `src/pet/palette.rs`
- Test: `src/pet/palette.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `Rgb { r: u8, g: u8, b: u8 }` (`src/pet/palette.rs:9`).
- Produces (test-only): `fn relative_luminance(c: Rgb) -> f32`, `fn contrast_ratio(a: Rgb, b: Rgb) -> f32`.

**Steps:**

- [ ] Add the failing test. In the `#[cfg(test)] mod tests` block of `src/pet/palette.rs` (after `per_pet_variety_within_species`, before the closing `}` at line 304), add:

```rust
    #[test]
    fn contrast_ratio_white_on_black_is_twenty_one() {
        let white = Rgb::new(255, 255, 255);
        let black = Rgb::new(0, 0, 0);
        let ratio = contrast_ratio(white, black);
        assert!(
            (ratio - 21.0).abs() < 0.1,
            "white-on-black contrast should be ~21:1, got {ratio}"
        );
    }

    #[test]
    fn contrast_ratio_is_symmetric_and_one_for_identical() {
        let c = Rgb::new(0x82, 0xbc, 0x83);
        assert!((contrast_ratio(c, c) - 1.0).abs() < 1e-4, "identical colors are 1:1");
        let d = Rgb::new(0x13, 0x11, 0x0f);
        assert!(
            (contrast_ratio(c, d) - contrast_ratio(d, c)).abs() < 1e-4,
            "contrast is symmetric"
        );
    }
```

- [ ] Run, expect FAIL: `cargo test --lib pet::palette::tests::contrast_ratio` → fails to compile with `cannot find function 'contrast_ratio' in this scope`.

- [ ] Add the implementation. In `src/pet/palette.rs`, immediately above the `#[cfg(test)] mod tests` line (currently line 183), add a non-test `pub(crate)` luminance fn used by both production (per-species eye-lightness selection in Task 5) and tests, plus the test-only ratio:

```rust
/// WCAG relative luminance of an sRGB color (0.0 black .. 1.0 white).
pub(crate) fn relative_luminance(c: Rgb) -> f32 {
    let chan = |v: u8| {
        let s = f32::from(v) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * chan(c.r) + 0.7152 * chan(c.g) + 0.0722 * chan(c.b)
}
```

Then inside `#[cfg(test)] mod tests`, add (the ratio is only needed by tests):

```rust
    fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
        let la = relative_luminance(a);
        let lb = relative_luminance(b);
        let (hi, lo) = if la >= lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }
```

- [ ] Run, expect PASS: `cargo test --lib pet::palette::tests::contrast_ratio` → 2 passed.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean (note: `relative_luminance` has a production caller in Task 5; until then it is `pub(crate)` and may warn as unused — if clippy flags it before Task 5, temporarily allow with a one-line `#[allow(dead_code)]` and remove that allow in Task 5. Prefer to land Tasks 1+5 close together to avoid the dance).

- [ ] Commit:
```bash
git checkout -b phase3-color-eyes-mood
git add src/pet/palette.rs
git commit -m "$(cat <<'EOF'
test: add relative-luminance contrast helper for eye/body floor

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 2 — Add the `particle` field and give particles their own hue

`role_color` maps `Particle => palette.accent` today (`palette.rs:133`). Add a dedicated `particle` color so particles read as their own species element.

**Files:**
- Modify: `src/pet/palette.rs`, `src/tui/panels/pet/colors.rs`
- Test: `src/pet/palette.rs`, `src/tui/panels/pet/colors.rs` are covered by their inline tests

**Interfaces:**
- Consumes: `ResolvedPalette { body, eye, mouth, accent, pattern }` (`palette.rs:117`), `role_color(role, &ResolvedPalette)` (`palette.rs:126`), `default_theme_palette()` (`palette.rs:138`), `resolve_pet_palette(species, &VisibleTraits)` (`palette.rs:165`).
- Produces: `ResolvedPalette.particle: Rgb`; `role_color(Particle) == palette.particle`.

**Steps:**

- [ ] Update the failing test. The existing `default_theme_matches_current_fixed_colors` (`palette.rs:242`) asserts `role_color(Particle, &p) == accent` on line 250. Change that line to assert the new dedicated particle color. First decide the default-theme particle color: the pre-color theme had no particle differentiation, so the default keeps it equal to accent for backward visual parity. Edit `palette.rs:250` from:

```rust
        assert_eq!(role_color(Particle, &p), Rgb::new(0xf0, 0xa6, 0x46));
```
to (unchanged value, but now asserts the new field round-trips through `role_color`):
```rust
        // Default theme keeps particle == accent for pre-color parity, but it is
        // now a dedicated field (role_color reads palette.particle, not accent).
        assert_eq!(role_color(Particle, &p), p.particle);
        assert_eq!(p.particle, Rgb::new(0xf0, 0xa6, 0x46));
```

- [ ] Add a failing test that particle is its own resolved hue (distinct from accent for live pets). In `#[cfg(test)] mod tests`, add:

```rust
    #[test]
    fn particle_is_its_own_species_hue() {
        use crate::pet::generation::Species;
        let p = resolve_pet_palette(Species::Crystal, &traits_with_hue(0));
        assert_ne!(
            p.particle, p.accent,
            "particle should resolve to its own hue, not reuse accent"
        );
    }
```

- [ ] Run, expect FAIL: `cargo test --lib pet::palette` → fails to compile (`no field 'particle' on type 'ResolvedPalette'`).

- [ ] Add the field. In `ResolvedPalette` (`palette.rs:117`):
```rust
pub struct ResolvedPalette {
    pub body: Rgb,
    pub eye: Rgb,
    pub mouth: Rgb,
    pub accent: Rgb,
    pub pattern: Rgb,
    pub particle: Rgb,
}
```

- [ ] Point `role_color` at it (`palette.rs:133`):
```rust
        PaletteRoleName::Particle => palette.particle,
```

- [ ] Populate `default_theme_palette` (`palette.rs:138`) — add `particle: Rgb::new(0xf0, 0xa6, 0x46),` after the `pattern` line so it equals accent (pre-color parity).

- [ ] Populate `resolve_pet_palette` (`palette.rs:174`). Give the particle its own hue offset (distinct from accent's `+120`). Add, inside the returned struct after `pattern`:
```rust
        particle: role(0.80, 0.20, h + 160.0),
```

- [ ] Fix the two consumers in `colors.rs` so they carry the field through. In `seed_pet_palette` (`colors.rs:307`) add after the `pet_pattern` line — wait: `SemanticStyles` has no `pet_particle` style; particle is rendered through `pet_accent` historically. Do NOT add a `SemanticStyles` field. Instead, `seed_pet_palette` returns a `SemanticStyles`, not a `ResolvedPalette`, so it is unaffected by the new field. The affected fn is `palette_from_styles` (`colors.rs:328`), which constructs a `ResolvedPalette` and now omits `particle`. Add to its returned struct:
```rust
        particle: rgb(styles.pet_accent, default.particle),
```
(particle snapshots from `pet_accent` because no dedicated particle `SemanticStyles` slot exists; this preserves the live dim/lift coherence for particle = accent-tracked, matching today's behavior where particle reused accent).

- [ ] Run, expect PASS: `cargo test --lib pet::palette` → all pass.

- [ ] Run, expect PASS for the colors module + the surface round-trip tests that build a `ResolvedPalette`:
```bash
cargo test --lib tui::panels::pet::colors
cargo test --lib menubar::render
cargo test --lib companion
```
(These build `default_theme_palette()` / `ResolvedPalette` and must still compile + pass.)

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/pet/palette.rs src/tui/panels/pet/colors.rs
git commit -m "$(cat <<'EOF'
feat: give pet particles their own resolved species hue

ResolvedPalette gains a dedicated `particle` field; role_color reads it
instead of reusing accent. Default theme keeps particle==accent for
pre-color parity.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 3 — Raise body chroma off the pinned ~0.10 (the `species_body_chroma` knob)

The bodies read near-grey because body chroma is pinned at `0.10` in `resolve_pet_palette` (`palette.rs:175`). Introduce a per-species chroma knob and raise it so the species hue registers, while keeping the OKLCH gamut-mapped resolution (`oklch_to_rgb` already chroma-reduces out-of-gamut requests, so a high request never panics or hue-shifts).

**Files:**
- Modify: `src/pet/palette.rs`
- Test: `src/pet/palette.rs`

**Interfaces:**
- Consumes: `oklch_to_rgb`, `resolve_pet_palette`, `species_base_hue`.
- Produces: `fn species_body_chroma(species: Species) -> f32`.

**Steps:**

- [ ] Add the failing test asserting bodies are now visibly chromatic (the OKLab a/b magnitude is non-trivial, not near-grey). In `#[cfg(test)] mod tests`, add (reuses the existing test-only `rgb_to_oklab_ab`):

```rust
    #[test]
    fn bodies_are_visibly_chromatic_not_grey() {
        use crate::pet::generation::Species;
        for s in Species::all() {
            let body = resolve_pet_palette(s, &traits_with_hue(0)).body;
            let (a, b) = rgb_to_oklab_ab(body);
            let chroma = (a * a + b * b).sqrt();
            assert!(
                chroma > 0.04,
                "{s:?} body reads near-grey (oklab chroma {chroma:.3}); raise species_body_chroma"
            );
        }
    }
```

(Note: the realized sRGB chroma after gamut mapping is below the OKLCH request; `> 0.04` is a conservative "clearly not grey" floor — the old pinned `0.10` request realized to roughly grey because lightness `0.74` + gamut clip flattened it for several hues. Verify the threshold empirically in the GREEN step; if a legitimately raised palette lands a hue just under `0.04`, raise that species' chroma rather than lowering the test floor.)

- [ ] Run, expect FAIL: `cargo test --lib pet::palette::tests::bodies_are_visibly_chromatic` → assertion fails for at least one species (the pinned `0.10` is too low for several family hues at L=0.74).

- [ ] Add the knob. In `src/pet/palette.rs`, after `species_base_hue` (ends line 160), add:

```rust
/// Per-species body chroma (OKLCH). Raised off the old pinned 0.10 so the
/// species hue actually registers. Soft-bodied/pale species (Crystal ice,
/// Ghost lavender) stay lower; saturated identities (Glitch acid, Mech amber)
/// go higher. `oklch_to_rgb` gamut-maps any out-of-gamut request, so these are
/// safe ceilings, not exact realized chroma.
fn species_body_chroma(species: Species) -> f32 {
    match species {
        Species::Fuzz => 0.13,    // peach
        Species::Blob => 0.14,    // mint
        Species::Ghost => 0.12,   // lavender (pale, keep soft)
        Species::Glitch => 0.18,  // acid/phosphor (loud)
        Species::Crystal => 0.11, // ice (cold, pale shell)
        Species::Mech => 0.15,    // amber/brass
    }
}
```

- [ ] Wire it into `resolve_pet_palette`. Replace the `body` line (`palette.rs:175`):
```rust
        body: role(0.74, 0.10, h),
```
with:
```rust
        body: role(0.74, species_body_chroma(species), h),
```

- [ ] Run, expect PASS: `cargo test --lib pet::palette::tests::bodies_are_visibly_chromatic` → passes. If any species fails, raise its `species_body_chroma` value (do NOT lower the test floor).

- [ ] Run the existing palette suite (the species-separation + per-pet-variety tests must still hold at higher chroma): `cargo test --lib pet::palette` → all pass.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/pet/palette.rs
git commit -m "$(cat <<'EOF'
feat: raise pet body chroma off pinned 0.10 via species_body_chroma

Bodies now carry visible species hue instead of reading near-grey;
gamut mapping in oklch_to_rgb keeps every request in range.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 4 — Retune `species_base_hue` toward the identity palette

The current hues (Fuzz 70 amber / Blob 195 teal / Ghost 300 violet / Glitch 135 acid / Crystal 230 ice / Mech 250 steel) don't match the spec identity table (peach / mint / lavender / acid / ice / amber). Re-point them, keeping the `resolve_pet_palette` OKLCH path and the ±18° per-seed jitter.

**Files:**
- Modify: `src/pet/palette.rs`
- Test: `src/pet/palette.rs`

**Interfaces:**
- Consumes/Produces: `fn species_base_hue(species: Species) -> f32` (signature retained from CONTRACT §2.7; values change).

**Steps:**

- [ ] Add a failing test pinning the new family hues (and that bodies stay distinct after retune). In `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn species_base_hues_match_identity_family() {
        use crate::pet::generation::Species;
        // OKLCH hue degrees (approx): peach ~40, mint ~150, lavender ~300,
        // acid ~135, ice ~230, amber ~75. Verify the family anchor, not exact
        // realized RGB (that depends on chroma/jitter).
        assert!((species_base_hue(Species::Fuzz) - 40.0).abs() < 1.0);
        assert!((species_base_hue(Species::Blob) - 150.0).abs() < 1.0);
        assert!((species_base_hue(Species::Ghost) - 300.0).abs() < 1.0);
        assert!((species_base_hue(Species::Glitch) - 135.0).abs() < 1.0);
        assert!((species_base_hue(Species::Crystal) - 230.0).abs() < 1.0);
        assert!((species_base_hue(Species::Mech) - 75.0).abs() < 1.0);
    }

    #[test]
    fn all_species_bodies_are_mutually_distinct() {
        use crate::pet::generation::Species;
        let bodies: Vec<_> = Species::all()
            .into_iter()
            .map(|s| resolve_pet_palette(s, &traits_with_hue(0)).body)
            .collect();
        for (i, a) in bodies.iter().enumerate() {
            for b in bodies.iter().skip(i + 1) {
                assert_ne!(a, b, "two species bodies collided after hue retune");
            }
        }
    }
```

- [ ] Run, expect FAIL: `cargo test --lib pet::palette::tests::species_base_hues_match_identity_family` → fails (current Fuzz=70, Blob=195, etc.).

- [ ] Retune `species_base_hue` (`palette.rs:151`):

```rust
fn species_base_hue(species: Species) -> f32 {
    match species {
        Species::Fuzz => 40.0,     // peach
        Species::Blob => 150.0,    // mint
        Species::Ghost => 300.0,   // lavender
        Species::Glitch => 135.0,  // acid/phosphor
        Species::Crystal => 230.0, // ice
        Species::Mech => 75.0,     // amber/brass
    }
}
```

- [ ] Run, expect PASS: `cargo test --lib pet::palette::tests::species_base_hues_match_identity_family` and `..::all_species_bodies_are_mutually_distinct` → pass. (Mech amber 75 and Fuzz peach 40 are close in hue but separated by chroma + the +35/+120 derived roles; if `all_species_bodies_are_mutually_distinct` ever fails because Fuzz/Mech bodies collide at a given seed_hue=0, nudge Mech to `78.0`. Verify in GREEN.)

- [ ] Run the full palette suite: `cargo test --lib pet::palette` → all pass.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/pet/palette.rs
git commit -m "$(cat <<'EOF'
feat: retune species_base_hue to the identity palette family

peach/mint/lavender/acid/ice/amber; keeps OKLCH resolution + seed jitter.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 5 — `eye_color_for_mood` + per-species resting-eye lightness + the ≥3:1 floor

The eye color is pinned to `EYE_HUE = 142.0` in `resolve_pet_palette` (`palette.rs:163/176`). Replace the pinned resting eye with a per-species resting green that clears ≥3:1 contrast against the (now-chromatic) body, and add the mood→eye-color data fn. This task delivers the resting-eye floor and the mood mapping data path; Task 6 hooks the data path into the per-tick render site.

**Files:**
- Modify: `src/pet/palette.rs`
- Test: `src/pet/palette.rs`

**Interfaces:**
- Consumes: `Mood` (`src/game/metabolism.rs:25` — `Happy, Ecstatic, Content, Hungry, Sad, Sleepy, Wilted`), `Rgb`, `oklch_to_rgb`, `relative_luminance` (Task 1), `resolve_pet_palette`.
- Produces (CONTRACT §2.6):
  ```rust
  pub fn eye_color_for_mood(mood: Mood) -> Rgb;
  pub fn apply_mood_eye_color(palette: &mut ResolvedPalette, mood: Mood);
  fn resting_eye_color(species: Species) -> Rgb; // per-species green clearing the floor
  ```

**Steps:**

- [ ] Add the failing contrast-floor test (the headline acceptance bar). In `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn resting_eye_clears_three_to_one_contrast_against_body() {
        use crate::pet::generation::Species;
        // Sweep seeds so the per-seed jittered body never sneaks under the floor.
        for s in Species::all() {
            for hue in (0..360).step_by(30) {
                let p = resolve_pet_palette(s, &traits_with_hue(hue));
                let ratio = contrast_ratio(p.eye, p.body);
                assert!(
                    ratio >= 3.0,
                    "{s:?} hue {hue}: resting eye/body contrast {ratio:.2} < 3.0"
                );
            }
        }
    }
```

- [ ] Add the failing mood-mapping test:

```rust
    #[test]
    fn eye_color_is_green_at_rest_and_shifts_with_mood() {
        use crate::game::metabolism::Mood;
        let rest = eye_color_for_mood(Mood::Content);
        assert!(
            rest.g > rest.r && rest.g > rest.b,
            "resting (Content) eye must read green, got {rest:?}"
        );
        // Excited -> warm/gold (red+green high, blue low; warmer than rest).
        let excited = eye_color_for_mood(Mood::Ecstatic);
        assert!(
            excited.r >= rest.r,
            "excited eye should warm toward gold (more red than rest)"
        );
        // Tired -> cool blue (blue dominates).
        let tired = eye_color_for_mood(Mood::Sleepy);
        assert!(
            tired.b > tired.r,
            "tired (Sleepy) eye should read cool/blue, got {tired:?}"
        );
        // Wilted -> desaturated/grey (channels close together).
        let wilted = eye_color_for_mood(Mood::Wilted);
        let spread = wilted.r.abs_diff(wilted.g).max(wilted.g.abs_diff(wilted.b));
        assert!(
            spread < 24,
            "wilted eye should desaturate toward grey, got spread {spread}"
        );
    }

    #[test]
    fn apply_mood_eye_color_overwrites_only_the_eye() {
        use crate::game::metabolism::Mood;
        use crate::pet::generation::Species;
        let mut p = resolve_pet_palette(Species::Blob, &traits_with_hue(7));
        let (body, mouth, accent, pattern, particle) =
            (p.body, p.mouth, p.accent, p.pattern, p.particle);
        apply_mood_eye_color(&mut p, Mood::Sleepy);
        assert_eq!(p.eye, eye_color_for_mood(Mood::Sleepy));
        assert_eq!((p.body, p.mouth, p.accent, p.pattern, p.particle),
                   (body, mouth, accent, pattern, particle),
                   "mood eye color must not touch any other role");
    }
```

- [ ] Run, expect FAIL: `cargo test --lib pet::palette::tests::eye_color_is_green_at_rest` → fails to compile (`eye_color_for_mood` not found).

- [ ] Add the mood→eye color data fn. In `src/pet/palette.rs`, replace the `EYE_HUE` constant + the `eye:` line in `resolve_pet_palette`. First, add an import at the top of the file (after `use crate::pet::render::PaletteRoleName;` at line 3):
```rust
use crate::game::metabolism::Mood;
```

- [ ] Delete the pinned constant (`palette.rs:162-163`):
```rust
/// Pinned green eye signature (same for every species).
const EYE_HUE: f32 = 142.0;
```
and add, after `species_body_chroma` (from Task 3):

```rust
/// Mood -> eye color (OKLCH-resolved). Green at rest, warming to gold when
/// excited, cooling to blue when tired, desaturating toward grey when wilted.
/// This is the eye-color half of the mood signal; the eye *glyph* is owned by
/// `expression_for` (render.rs) and updates on the same animation tick.
pub fn eye_color_for_mood(mood: Mood) -> Rgb {
    // (lightness, chroma, hue) tuned so each clears the resting green floor's
    // intent while staying calm (no neon). Wilted drops chroma to near-grey.
    let (l, c, h) = match mood {
        Mood::Content => (0.82, 0.19, 145.0), // resting green
        Mood::Happy => (0.84, 0.20, 130.0),   // brighter green, a touch warm
        Mood::Ecstatic => (0.86, 0.20, 95.0), // warm gold-green
        Mood::Hungry => (0.80, 0.18, 70.0),   // amber-warm (seeking)
        Mood::Sad => (0.74, 0.14, 250.0),     // cool, muted blue
        Mood::Sleepy => (0.78, 0.15, 250.0),  // cool blue, tired
        Mood::Wilted => (0.70, 0.03, 145.0),  // desaturated grey-green
    };
    oklch_to_rgb(l, c, h)
}

/// Overwrite only the eye role with the mood-driven color. Hooked at the
/// per-tick render site (rerender_pet_for_view_model) so eye color rides mood
/// without lagging the eye glyph.
pub fn apply_mood_eye_color(palette: &mut ResolvedPalette, mood: Mood) {
    palette.eye = eye_color_for_mood(mood);
}
```

- [ ] Make `resolve_pet_palette` build the resting (Content) eye through the per-species floor. The resting eye must clear ≥3:1 against the body; a flat green at L=0.82 fails on Blob (mint) / Fuzz (peach) / Glitch (acid) per the spec. Add a per-species resting-eye resolver after `eye_color_for_mood`:

```rust
/// Per-species RESTING eye color (Mood::Content), shifted in lightness so it
/// clears the >=3:1 luminance floor against that species' body. The green
/// anchor at L=0.82 collides with mint/peach/acid bodies; those species get a
/// darker (or, for already-dark bodies, lighter) eye so the floor holds.
fn resting_eye_color(species: Species) -> Rgb {
    // Default resting green; shift lightness per species to clear the floor.
    let l = match species {
        // Light-bodied greens/peaches: darken the eye for separation.
        Species::Blob | Species::Glitch => 0.62,
        Species::Fuzz => 0.66,
        // Lavender/ice/amber bodies already separate from green; keep bright.
        Species::Ghost | Species::Crystal | Species::Mech => 0.82,
    };
    oklch_to_rgb(l, 0.19, 145.0)
}
```

- [ ] Replace the `eye:` line in `resolve_pet_palette` (`palette.rs:176`):
```rust
        eye: oklch_to_rgb(0.82, 0.19, EYE_HUE),
```
with:
```rust
        eye: resting_eye_color(species),
```

- [ ] Run, expect PASS: `cargo test --lib pet::palette::tests::resting_eye_clears_three_to_one` and `..::eye_color_is_green_at_rest` and `..::apply_mood_eye_color_overwrites_only_the_eye` → pass.
  - If `resting_eye_clears_three_to_one_contrast_against_body` still fails for a species, tighten that species' `resting_eye_color` lightness (darker for light bodies, lighter for dark bodies) until the floor holds across the full seed sweep. Do NOT relax the `>= 3.0` test. Verify empirically in GREEN.

- [ ] Rewrite `eyes_are_green_for_every_species` to "green at rest" (CONTRACT §2.6). The current test (`palette.rs:276`) asserts every species' eye equals one fixed green. After the per-species resting shift, eyes are green- at-rest but per-species. Replace the test body:

```rust
    #[test]
    fn eyes_are_green_at_rest_for_every_species() {
        use crate::pet::generation::Species;
        // At rest (the resting eye is Mood::Content's color baked into resolve),
        // every species' eye reads green (g dominates) even if lightness differs.
        for s in Species::all() {
            let eye = resolve_pet_palette(s, &traits_with_hue(123)).eye;
            assert!(
                eye.g > eye.r && eye.g > eye.b,
                "{s:?} resting eye not green: {eye:?}"
            );
        }
    }
```

- [ ] Run, expect PASS: `cargo test --lib pet::palette` → all pass (the old `eyes_are_green_for_every_species` is now `eyes_are_green_at_rest_for_every_species`).

- [ ] Confirm `relative_luminance` now has a production caller (it backs nothing in production yet — it is used only by the test `contrast_ratio`). Decision: keep `relative_luminance` `pub(crate)` because Task 6 / dev_preview review and any future flat-color check use it, but to satisfy `clippy --all-targets -D warnings` it MUST have a non-test caller OR be `#[cfg(test)]`. Since Phase 3 has no production luminance caller, move `relative_luminance` under `#[cfg(test)]` (revert the `pub(crate)` from Task 1):
  - Change its signature from `pub(crate) fn relative_luminance` to `#[cfg(test)] fn relative_luminance` and move it into the `#[cfg(test)] mod tests` block (next to `contrast_ratio`). Remove any `#[allow(dead_code)]` added in Task 1.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/pet/palette.rs
git commit -m "$(cat <<'EOF'
feat: mood-driven eye color + per-species resting-eye contrast floor

eye_color_for_mood maps green-rest/warm-excited/blue-tired/grey-wilted;
resting eye is per-species lightness-shifted to clear >=3:1 vs body for the
green-colliding species (Blob/Fuzz/Glitch). EYE_HUE constant removed;
eyes_are_green_for_every_species -> green-at-rest.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 6 — Hook the mood→eye color into the per-tick render site

Mutate `vm.pet_palette.eye` from `vm.pet_render.mood` inside `rerender_pet_for_view_model` (`watch.rs:514`) so the watch, menubar, and companion (all of which call this fn and read `vm.pet_palette` for eye color) get mood-colored eyes at animation-tick cadence — matching the eye *glyph* that `expression_for` already sets per tick.

**Files:**
- Modify: `src/commands/watch.rs`
- Test: `src/commands/watch.rs` (inline test) or `tests/presentation_pet.rs` if a watch-VM fixture exists there

**Interfaces:**
- Consumes: `apply_mood_eye_color(&mut ResolvedPalette, Mood)` (Task 5), `eye_color_for_mood` (Task 5), `WatchViewModel { pet_palette, pet_render: PetRenderModel { mood, .. } }` (`watch.rs:222-231`), `rerender_pet_for_view_model(vm, tick, hold_eyes_closed)` (`watch.rs:514`).
- Produces: the mood→eye color contract is now honored on every live surface routing through `rerender_pet_for_view_model`.

**Steps:**

- [ ] Find the existing watch-VM test harness. Run:
```bash
grep -rn "build_watch_view_model_for_test\|rerender_pet_for_view_model" src/commands/watch.rs tests/*.rs
```
Use `build_watch_view_model_for_test_at` (`watch.rs:373`) to build a real VM in an inline `#[cfg(test)]` test in `watch.rs`, then mutate `vm.pet_render.mood` and assert the eye color tracks. (A fixed seed + tempdir DB is needed; mirror the setup in `tests/presentation_pet.rs`.)

- [ ] Add the failing test. Append to the `#[cfg(test)] mod tests` block in `src/commands/watch.rs` (create one if absent). It builds a VM, forces two moods, and asserts `vm.pet_palette.eye` matches `eye_color_for_mood(mood)` after `rerender_pet_for_view_model`:

```rust
    #[test]
    fn rerender_applies_mood_eye_color() {
        use crate::game::metabolism::Mood;
        use crate::pet::palette::eye_color_for_mood;

        let dir = tempfile::tempdir().unwrap();
        let usage_db = dir.path().join("usage.sqlite");
        // Minimal state with a known seed; reuse the test seed helper if present,
        // else construct a PetState via the existing init path in this module's
        // tests. (See sibling tests for the exact constructor.)
        let state = crate::commands::watch::tests::sample_pet_state("phase3-eye-seed");
        let mut vm = build_watch_view_model_for_test_at(
            &state,
            &usage_db,
            time::OffsetDateTime::now_utc(),
        )
        .unwrap();

        vm.pet_render.mood = Mood::Sleepy;
        rerender_pet_for_view_model(&mut vm, 1, false).unwrap();
        assert_eq!(
            vm.pet_palette.eye,
            eye_color_for_mood(Mood::Sleepy),
            "sleepy mood should cool the eye color"
        );

        vm.pet_render.mood = Mood::Ecstatic;
        rerender_pet_for_view_model(&mut vm, 2, false).unwrap();
        assert_eq!(
            vm.pet_palette.eye,
            eye_color_for_mood(Mood::Ecstatic),
            "ecstatic mood should warm the eye color"
        );
    }
```
  - If `src/commands/watch.rs` has no test-state helper, build `state` with the same constructor used by an existing watch test (search `let state = ` near `build_watch_view_model_for_test`); copy that exact setup rather than inventing one. Do NOT mock the DB — use a real empty `tempdir` SQLite path, which `build_watch_view_model_for_test_at` opens.

- [ ] Run, expect FAIL: `cargo test --lib commands::watch::tests::rerender_applies_mood_eye_color` → fails (eye stays at the resting per-species color; mood not applied).

- [ ] Add the hook. In `rerender_pet_for_view_model` (`watch.rs:514`), at the top of the body (before `let species = ...` at line 519), add:

```rust
    // Eye color rides mood at the same cadence as the eye glyph (expression_for),
    // overwriting only the eye role. The ~10s worker palette rebuild stays
    // mood-blind; this per-tick site owns mood -> eye color.
    crate::pet::palette::apply_mood_eye_color(&mut vm.pet_palette, vm.pet_render.mood);
```

- [ ] Run, expect PASS: `cargo test --lib commands::watch::tests::rerender_applies_mood_eye_color` → passes.

- [ ] Verify the menubar / companion get it for free (both call `rerender_pet_for_view_model` and read `vm.pet_palette` for the eye via `role_color`). No code change needed there. Add a one-line note in the commit body. Run their suites to confirm no regression:
```bash
cargo test --lib menubar
cargo test --lib companion
```

- [ ] Companion redraw gotcha (document, do not fix): `advance_companion_animation` (`companion/app.rs:303`) diffs on `pet_art`/`pet_spans`/`breath_offset_y`, NOT `pet_palette`. A mood change always changes the eye *glyph* (`expression_for`), so `pet_spans`/`pet_art` differ and the redraw fires; a color-only change with an identical glyph would not trigger a redraw, but that combination does not occur because mood drives both. No action; note it in the commit body.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/commands/watch.rs
git commit -m "$(cat <<'EOF'
feat: ride pet eye color on mood at animation-tick cadence

rerender_pet_for_view_model now applies apply_mood_eye_color before
re-render, so watch/menubar/companion (all routing through it) color the
eye by mood in lockstep with the eye glyph. Companion redraw still keys on
the glyph diff, which always changes when mood does.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 7 — Apply mood eye color in the dev-preview pets scenario

Phase 2 extended `src/dev_preview/pets.rs` to render the full mood set per adult stage (CONTRACT preview-lab note). Those cells build the palette via `resolve_pet_palette` (resting eye only). Apply `apply_mood_eye_color` per cell so the preview mood set actually shows the mood eye colors — the visual regression surface for this phase.

**Files:**
- Modify: `src/dev_preview/pets.rs`
- Test: `src/dev_preview/pets.rs` (inline) + `cargo test --features dev-preview --test dev_preview`

**Interfaces:**
- Consumes: `apply_mood_eye_color(&mut ResolvedPalette, Mood)` (Task 5), `resolve_pet_palette`, `render_pet`, `pet_role_spans_for_line`.

**Steps:**

- [ ] Confirm the Phase 2 mood-set cell renderer. Run:
```bash
grep -n "Mood::\|resolve_pet_palette\|render_pet_cell\|render_glitch_state_cell\|mood" src/dev_preview/pets.rs
```
The mood-bearing cell render fns are `render_pet_cell` (`pets.rs:84`, currently hard-codes `Mood::Content`) and `render_glitch_state_cell` (`pets.rs:182`, uses `fixture.mood`). Phase 2 may have added a mood-matrix renderer; the hook is the same in every one: after `let palette = ...resolve_pet_palette(...)`, apply the mood.

- [ ] Add a failing test that the glitch mood fixtures produce distinct eye colors (the preview must visibly differentiate moods). In `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn glitch_state_fixtures_apply_mood_eye_color() {
        use crate::pet::palette::{eye_color_for_mood, resolve_pet_palette};
        use crate::pet::generation::{generate_pet, Species};
        let pet = generate_pet("glorp-preview-glitch-live").with_species(Species::Glitch);
        let mut content = resolve_pet_palette(Species::Glitch, &pet.traits);
        let mut ecstatic = content;
        crate::pet::palette::apply_mood_eye_color(&mut content, Mood::Content);
        crate::pet::palette::apply_mood_eye_color(&mut ecstatic, Mood::Ecstatic);
        assert_ne!(
            content.eye, ecstatic.eye,
            "preview mood fixtures must show distinct eye colors"
        );
        assert_eq!(content.eye, eye_color_for_mood(Mood::Content));
        assert_eq!(ecstatic.eye, eye_color_for_mood(Mood::Ecstatic));
    }
```

- [ ] Run, expect FAIL only if the apply step is missing in the render path — this test exercises the palette fn directly so it passes already; its purpose is to lock the contract. Run: `cargo test --lib dev_preview::pets::tests::glitch_state_fixtures_apply_mood_eye_color` → PASS (it validates Task 5's fns). Keep it as a regression guard.

- [ ] Apply the mood in `render_glitch_state_cell` (`pets.rs:185`). After:
```rust
    let palette = crate::pet::palette::resolve_pet_palette(Species::Glitch, &pet.traits);
```
add:
```rust
    let mut palette = palette;
    crate::pet::palette::apply_mood_eye_color(&mut palette, fixture.mood);
```
(make `palette` mutable; `pet_role_spans_for_line` takes `&palette`).

- [ ] Apply the mood in `render_pet_cell` (`pets.rs:93`) and any Phase-2 mood-matrix cell renderer. For `render_pet_cell` the mood is `Mood::Content` (line 97), so after:
```rust
    let palette = crate::pet::palette::resolve_pet_palette(species, &pet.traits);
```
add:
```rust
    let mut palette = palette;
    crate::pet::palette::apply_mood_eye_color(&mut palette, Mood::Content);
```
For any Phase-2 renderer that takes a `mood` parameter, apply `apply_mood_eye_color(&mut palette, mood)` identically. (Search the file; if Phase 2 added a `render_pet_mood_cell` or similar, patch it the same way.)

- [ ] Run, expect PASS: `cargo test --lib dev_preview::pets` → all pass.

- [ ] Run the preview-lab integration suite: `cargo test --features dev-preview --test dev_preview` → pass (no manifest-schema change is introduced by Phase 3; Phase 2 owns the schema bump for the mood set — do NOT bump it here).

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/dev_preview/pets.rs
git commit -m "$(cat <<'EOF'
feat: apply mood eye color in dev-preview pet cells

Preview mood set now shows the mood-driven eye colors, making the eye-mood
signal reviewable in the preview lab.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 8 — Delete the dead `palette_roles` path (the second pinned eye hue)

`render.rs::palette_roles` (`render.rs:161`) and its `PaletteRoles`/`PaletteRole` types + `role`/`with_saturation` helpers have zero production callers (only the `tests/generation.rs::palette_roles_follow_tokenpet_hue_offsets` test exercises them). It carries a duplicate, divergent eye hue (`offset 180`, chroma `0.13`). With the mood→eye color path owning eye color, this dead second eye-hue source is removed (CONTRACT §2.6: "adopt or delete, both change").

**Files:**
- Modify: `src/pet/render.rs`, `tests/generation.rs`
- Test: `tests/generation.rs` (the dead-code test is removed)

**Interfaces:**
- Removes: `pub fn palette_roles`, `pub struct PaletteRoles`, `pub struct PaletteRole`, `fn role`, `impl PaletteRole { fn with_saturation }`.
- Must NOT remove: `PaletteRoleName` (live, used everywhere), `species_animation_profile`, `AnimationProfile` (Phase 4 deletes the dead breath fields — leave the type alone here).

**Steps:**

- [ ] Confirm zero production callers. Run:
```bash
grep -rn "palette_roles\|PaletteRoles\b\|PaletteRole\b" src/ tests/
```
Expect hits only in `src/pet/render.rs` (definitions + the dead `role`/`with_saturation`) and `tests/generation.rs` (the test). If any `src/` file outside `render.rs` references `palette_roles`/`PaletteRoles`/`PaletteRole`, STOP — it is not dead; report and do not delete.

- [ ] Remove the dead-code test first (so the suite still compiles after the type removal). In `tests/generation.rs`, delete the entire `palette_roles_follow_tokenpet_hue_offsets` test (`tests/generation.rs:214-227`) and remove any now-unused `palette_roles`/`PaletteRoles` import at the top of that file (re-run the grep above to find the import line; delete only the unused symbols).

- [ ] Delete `palette_roles` (`render.rs:161-171`), the `PaletteRoles` struct (`render.rs:85-92`), the `PaletteRole` struct (`render.rs:94-100`), the `role` fn (`render.rs:230-244`), and the `impl PaletteRole { fn with_saturation }` block (`render.rs:246-251`).

- [ ] Run, expect FAIL→PASS cycle: `cargo build` → compiles clean (no remaining references). If the compiler reports a still-live reference, the symbol was not dead — STOP and report.

- [ ] Run the suites: `cargo test --lib pet::render` and `cargo test --test generation` → pass.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean (this is the step that confirms no dead-code warnings remain).

- [ ] Commit:
```bash
git add src/pet/render.rs tests/generation.rs
git commit -m "$(cat <<'EOF'
refactor: delete dead palette_roles path and its divergent eye hue

palette_roles/PaletteRoles/PaletteRole had zero production callers and a
second, divergent eye-hue source. Eye color is now owned by the mood->eye
data path. Removed the dead-code-only test that pinned it.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 9 — Re-validate the live lift chain at higher chroma (saturating_add blow-out)

`seed_pet_palette` → live dim/lift/shimmer chain → `palette_from_styles` round-trips per-channel via `saturating_add` (`colors.rs:160-166, 182-188`). At the old chroma a single high channel rarely hit 255; at the new higher chroma it can, hue-shifting the lift. Add a guard test; only change code if it fails (YAGNI — `saturating_add` is the existing contract; do not rewrite it unless a test proves a visible hue break).

**Files:**
- Modify (only if test fails): `src/tui/panels/pet/colors.rs`
- Test: `src/tui/panels/pet/colors.rs` (inline)

**Interfaces:**
- Consumes: `resolve_pet_palette`, `seed_pet_palette`, `activity_lift_style`, `palette_from_styles`.

**Steps:**

- [ ] Add the guard test. In the `#[cfg(test)] mod tests` of `colors.rs` (add the module if absent — match the file's existing `pub(super)`/`pub(crate)` visibility and import `semantic_styles`):

```rust
    #[test]
    fn activity_lift_does_not_invert_body_hue_at_high_chroma() {
        use crate::pet::generation::Species;
        use crate::pet::palette::resolve_pet_palette;
        // The loudest body (Glitch acid). Lift it hard and confirm green still
        // dominates (no channel pins to 255 and flips the hue read).
        let palette = resolve_pet_palette(Species::Glitch, &crate::pet::generation::VisibleTraits {
            eyes: "o o".into(), mouth: "w".into(), pattern: "...".into(), accent: "*".into(),
            palette_index: 0, morph_index: 0, morph_pup_index: 0, seed_hue: 0, saturation_percent: 100,
        });
        let body_before = palette.body;
        assert!(body_before.g >= body_before.r && body_before.g >= body_before.b,
            "glitch body should be green-dominant before lift: {body_before:?}");
        let styles = seed_pet_palette(&crate::tui::style::semantic_styles(), &palette);
        let lifted = activity_lift_style(styles.pet_body, 2.0, ColorCapability::Truecolor);
        if let Some(ratatui::style::Color::Rgb(r, g, b)) = lifted.fg {
            assert!(g >= r && g >= b,
                "max activity lift must not flip glitch body off green: ({r},{g},{b})");
        } else {
            panic!("lifted body should stay Rgb");
        }
    }
```

- [ ] Run: `cargo test --lib tui::panels::pet::colors::tests::activity_lift_does_not_invert_body_hue_at_high_chroma`.
  - Expect PASS in most cases (green-dominant body + uniform lift keeps green ≥ others). If it FAILS (a high channel pins to 255 and another overtakes green), apply the minimal fix: clamp the lift uniformly by the headroom of the *brightest* channel so the per-channel ratio is preserved. Replace the `activity_lift_style` Rgb arm (`colors.rs:159-165`) with a headroom-capped uniform add:
    ```rust
        match style.fg {
            Some(Color::Rgb(r, g, b)) => {
                let headroom = 255u8.saturating_sub(r.max(g).max(b));
                let add = lift.min(headroom);
                style.fg(Color::Rgb(
                    r.saturating_add(add),
                    g.saturating_add(add),
                    b.saturating_add(add),
                ))
            }
            _ => style,
        }
    ```
    Then re-run the test → PASS. (Only do this if the guard test fails; otherwise leave `activity_lift_style` untouched.)

- [ ] Run the colors suite: `cargo test --lib tui::panels::pet::colors` → pass.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/tui/panels/pet/colors.rs
git commit -m "$(cat <<'EOF'
test: guard live lift chain against hue inversion at higher chroma

Adds a regression test that max activity lift keeps the loudest (Glitch)
body hue-stable. (Code unchanged unless the guard proved a break.)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Task 10 — Flat-color figure-ground acceptance check for Blob/Ghost

CONTRACT §1 + spec require a flat-color figure-ground check for the soft-bodied species (Blob/Ghost): with color off, the body must read as a solid creature, denser than the habitat dot field. Under `ColorCapability::Flat` the pet renders monochrome, so legibility is carried by glyph density, not color. This check verifies the *rendered* body still has enough dense (`▒▓█`) cells to read as figure when color is stripped.

**Files:**
- Test: `src/pet/render.rs` (inline `#[cfg(test)]`) — pure render check, no production change

**Interfaces:**
- Consumes: `render_pet`, `stage_template_lines` (via `render_pet`), Phase 2's Blob/Ghost silhouettes.

**Steps:**

- [ ] Add the failing/guard test. In the `#[cfg(test)] mod tests` of `src/pet/render.rs`:

```rust
    #[test]
    fn soft_bodies_read_as_figure_with_color_off() {
        use crate::pet::generation::Species;
        // Flat color strips hue; legibility must come from dense glyphs. The
        // body of a soft species must contain enough medium/dark shade cells
        // (▒▓█) to read as a solid creature, not a sparse dot cloud.
        let dense: &[char] = &['\u{2592}', '\u{2593}', '\u{2588}']; // ▒ ▓ █
        for species in [Species::Blob, Species::Ghost] {
            let pet = generate_pet("figure-ground-seed").with_species(species);
            // S4 is the first mass stage; soft bodies must already read solid.
            let rendered = render_pet(&pet, Stage::S4, Mood::Content, AnimationFrame {
                tick: 0,
                blink_suppression_ticks: 0,
                hold_eyes_closed: false,
                blink_slowdown: 0,
                soft_eyes: false,
                work_accent: WorkAccent::None,
            });
            let dense_cells = rendered
                .lines
                .iter()
                .flat_map(|l| l.chars())
                .filter(|c| dense.contains(c))
                .count();
            assert!(
                dense_cells >= 6,
                "{species:?} S4 body has only {dense_cells} dense (▒▓█) cells; \
                 a soft body must read as figure with color off"
            );
        }
    }
```

- [ ] Run: `cargo test --lib pet::render::tests::soft_bodies_read_as_figure_with_color_off`.
  - Expect PASS if Phase 2's Blob/Ghost S4 silhouettes carry a `▒▓` core (the spec mandates the Blob `▒▓` core for exactly this). If it FAILS, the Phase 2 silhouette is too sparse — this is a Phase 2 art bug surfaced here. STOP and report it (do not patch art in Phase 3; per the CONTRACT, silhouettes are Phase 2's deliverable). The `>= 6` floor is conservative; if Phase 2 art is correct it clears easily.

- [ ] Run the gate: `cargo clippy --all-targets --all-features -- -D warnings` → clean.

- [ ] Commit:
```bash
git add src/pet/render.rs
git commit -m "$(cat <<'EOF'
test: flat-color figure-ground check for Blob/Ghost soft bodies

Asserts the soft-bodied species carry enough dense (▒▓█) cells to read as
a solid creature when color is stripped (ColorCapability::Flat).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

## Final verification (run before declaring Phase 3 done)

- [ ] Full suite green: `cargo test` → 0 failures.
- [ ] Format clean: `cargo fmt --check`.
- [ ] Lint gate clean: `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Preview-lab visual review: `cargo run -- dev-preview --scenario pets --out target/glorp-preview-phase3 && open target/glorp-preview-phase3/index.html` — confirm by eye: (1) six distinct species body identities (peach/mint/lavender/acid/ice/amber), (2) particles read as their own element, (3) the mood set shows green-rest / warm-excited / blue-tired / grey-wilted eyes, (4) Blob/Ghost read as solid in the Flat matrix, (5) resting eyes are legible (not invisible) against every body.
- [ ] Roster still renders live: `GLORP_CONFIG_DIR=$(mktemp -d) cargo run -- init --yes --seed phase3-smoke --name buddy && GLORP_CONFIG_DIR=<same dir> cargo run -- status` — pet renders without crash or blank.
- [ ] Confirm identity untouched: no edits to `state.json` schema, `seed`/`accepted_name`/`xp`/vitals/stage/calibration. (Grep your diff: `git diff main --stat` should touch only `palette.rs`, `render.rs`, `colors.rs`, `watch.rs`, `dev_preview/pets.rs`, `tests/generation.rs`.)

## Notes on cross-phase reconciliation (for the reconciler)

- **Depends on Phase 1/2:** Task 10's figure-ground floor and Task 5's contrast floor assume Phase 2's 42 silhouettes + standardized mood faces exist. If Phase 2's Blob/Ghost S4 lacks a `▒▓` core, Task 10 fails and must be fixed in Phase 2, not here.
- **Exposes to Phase 4:** `ResolvedPalette.particle` field + `role_color(Particle)` plumbing, and the single eye-color hook in `rerender_pet_for_view_model`. Phase 4's `PaletteRoleName::Corruption` joins the same `role_color`/`colors.rs`/`style.rs` surfaces and must keep the "never touch eye-center" rule — the eye color is now a one-line hook, so corruption z-order over the eye glyph does not need to re-derive eye color.
- **Preview-lab schema:** Phase 3 does NOT bump the manifest schema (currently v3). Phase 2 owns the mood-set schema increment; Phase 3 only applies `apply_mood_eye_color` inside the existing cell renderers.
- **AnimationProfile left intact:** Task 8 deletes `palette_roles`/`PaletteRoles`/`PaletteRole` only. The dead `AnimationProfile.breath_period`/`breath_hold` fields are Phase 4's deletion (CONTRACT §3a) — do not touch `AnimationProfile` here.
