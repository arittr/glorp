# Render Seam — Plan 01: Color Resolution Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the three independent role→color paths (watch, companion, menubar) with one shared resolver plus a per-surface `SurfaceStyle`, with zero visible change to any surface.

**Architecture:** Relocate the pure per-channel color math out of `src/tui/panels/pet/colors.rs` into a neutral `src/presentation/color_ops.rs`. Introduce `SurfaceStyle` (per-surface policy), `LiveColorInputs` (per-frame live inputs), and `ResolvedColors` (per-role `Rgb` + eye emphasis) in `src/presentation/`. Implement `resolve_pet_colors(base, inputs, style)` that composes the channel ops in the exact current watch order — `[source_accent] → [phase_tint] → [energy_droop] → [shimmer] → [activity_lift]` — each gated by a `SurfaceStyle` knob. Route all three surfaces through it. The `dev-preview` goldens and hand-derived characterization tests prove byte-stability.

**Tech Stack:** Rust, ratatui (`Style`/`Color`/`Modifier`), the existing `crate::pet::palette` (`Rgb`, `ResolvedPalette`, `role_color`).

This is **Plan 01** of the render-seam re-architecture (`docs/superpowers/specs/2026-06-22-glorp-pet-scene-render-seam-design.md`, Tracks 0–1). Later plans cover effects-as-data (Track 2), `SceneDrawList` + companion migration (Track 3), watch/menubar adapters, the screen window, and dev-preview unification.

## Global Constraints

- **`src/pet/render.rs` and `src/pet/art.rs` are FROZEN.** The seam lives strictly above `render_pet`; do not modify the 11×8 templates or `render_pet`. (Spec: Migration rules.)
- **`src/presentation/` must not depend on `tui::component::TargetPath`** or any watch-specific id. (Spec: 06-15 neutral-id constraint.)
- **`dev-preview` goldens are the regression net.** Regenerate only via the `dev-preview` command, only into `.glorp-preview`-owned output. In this plan, watch/round goldens must come out **byte-identical** — any diff is a bug in the refactor, not an intentional re-bake.
- **No behavior change in this plan.** Every surface renders identically before and after.
- **Lint gates must stay clean at every commit:** `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- **Commit per task.**

## File Structure

- **Create** `src/presentation/color_ops.rs` — neutral, pure per-channel color math (`warm_shift`, `cool_shift`, `dim_shift`, `tint_for_phase`, `brighten_channel`, `darken_channel`, `activity_lift_channel`, `blend`). Operates on `crate::pet::palette::Rgb`. No ratatui, no tui imports.
- **Create** `src/presentation/surface.rs` — `SurfaceStyle`, `EyeEmphasis`, the four policy constants, `LiveColorInputs`, `ResolvedColors`, and `resolve_pet_colors`.
- **Modify** `src/presentation/mod.rs` — declare the two new modules, re-export the public types.
- **Modify** `src/tui/panels/pet/colors.rs` — re-point the `SemanticStyles`-level helpers at `color_ops` (keep them as thin ratatui wrappers); the watch adapter (Task 6) calls `resolve_pet_colors`.
- **Modify** `src/tui/panels/pet.rs:394-447` (`render_pet_inside`) — replace the inline 5-step `SemanticStyles` pipeline with one `resolve_pet_colors(WATCH_STYLE)` call mapped to `SemanticStyles`.
- **Modify** `src/companion/app.rs:504-510` (`pet_role_color`) — resolve via `resolve_pet_colors(ROUND_STYLE)`.
- **Modify** `src/menubar/render.rs:83-109` (`role_color_for_profile`) — resolve via `resolve_pet_colors(MENU_STYLE)`.
- **Test:** unit tests colocated in `color_ops.rs` / `surface.rs`; integration safety net = existing `dev-preview` goldens.

---

### Task 1: Neutral color-ops module

Move the pure per-channel math out from behind `pub(super)` in `colors.rs` so the resolver (in `presentation/`) can compose it. Behavior-preserving: `colors.rs` keeps its public functions but delegates.

**Files:**
- Create: `src/presentation/color_ops.rs`
- Modify: `src/presentation/mod.rs`
- Modify: `src/tui/panels/pet/colors.rs:46-118` (delegate `blend_colors`, `warm_shift`, `dim_shift`, `cool_shift`, `tint_style_for_phase`)
- Test: in `src/presentation/color_ops.rs`

**Interfaces:**
- Produces: `pub fn warm_shift(c: Rgb, amt: f32) -> Rgb`, `pub fn cool_shift(c: Rgb, amt: f32) -> Rgb`, `pub fn dim_shift(c: Rgb, amt: f32) -> Rgb`, `pub fn tint_for_phase(c: Rgb, phase: DayPhase, blend: f32) -> Rgb`, `pub fn brighten_channel(c: Rgb, mult: f32) -> Rgb`, `pub fn darken_channel(c: Rgb, mult: f32) -> Rgb`, `pub fn activity_lift_channel(c: Rgb, activity_level: f32) -> Rgb`, `pub fn blend(primary: Rgb, secondary: Rgb, primary_weight: f32) -> Rgb`. All operate on `crate::pet::palette::Rgb`.
- Consumes: `crate::pet::palette::Rgb`, `crate::tui::day::DayPhase`.

- [ ] **Step 1: Write the failing test**

In `src/presentation/color_ops.rs`, port the exact numeric behavior from `colors.rs` (verified against the current source) as the spec for the ops:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pet::palette::Rgb;
    use crate::tui::day::DayPhase;

    #[test]
    fn warm_shift_adds_red_subtracts_blue() {
        // amount 1.0 => +40 red (saturating), -30 blue (saturating), green unchanged
        assert_eq!(warm_shift(Rgb::new(100, 100, 100), 1.0), Rgb::new(140, 100, 70));
        assert_eq!(warm_shift(Rgb::new(250, 0, 10), 1.0), Rgb::new(255, 0, 0)); // saturates
    }

    #[test]
    fn dim_shift_scales_by_one_minus_half_amount() {
        // m = 1.0 - amount*0.5; amount 1.0 => 0.5x
        assert_eq!(dim_shift(Rgb::new(200, 100, 80), 1.0), Rgb::new(100, 50, 40));
    }

    #[test]
    fn darken_channel_clamps_multiplier_0_to_1() {
        assert_eq!(darken_channel(Rgb::new(200, 100, 80), 0.5), Rgb::new(100, 50, 40));
        assert_eq!(darken_channel(Rgb::new(200, 100, 80), 2.0), Rgb::new(200, 100, 80)); // clamp to 1.0
    }

    #[test]
    fn brighten_channel_caps_at_255() {
        assert_eq!(brighten_channel(Rgb::new(200, 100, 80), 1.4), Rgb::new(255, 140, 112));
    }

    #[test]
    fn activity_lift_adds_level_times_22_saturating() {
        // lift = (level.clamp(0,2) * 22) as u8; level 2.0 => +44
        assert_eq!(activity_lift_channel(Rgb::new(100, 100, 100), 2.0), Rgb::new(144, 144, 144));
        assert_eq!(activity_lift_channel(Rgb::new(250, 250, 250), 2.0), Rgb::new(255, 255, 255));
    }

    #[test]
    fn phase_tint_matches_legacy_curve() {
        let c = Rgb::new(120, 120, 120);
        assert_eq!(tint_for_phase(c, DayPhase::Day, 1.0), c); // day = identity
        assert_eq!(tint_for_phase(c, DayPhase::Dusk, 1.0), warm_shift(c, 0.18));
        assert_eq!(tint_for_phase(c, DayPhase::Dawn, 1.0), warm_shift(c, 0.10));
        assert_eq!(tint_for_phase(c, DayPhase::Night, 1.0), dim_shift(cool_shift(c, 0.18), 0.28));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib presentation::color_ops`
Expected: FAIL — module/functions not defined.

- [ ] **Step 3: Write minimal implementation**

In `src/presentation/color_ops.rs`, port each function from `colors.rs` to operate on `Rgb` instead of ratatui `Style`/`Color` (same arithmetic, verified against `colors.rs:46-118,159-167,181-188,271-292`):

```rust
use crate::pet::palette::Rgb;
use crate::tui::day::DayPhase;

pub fn blend(primary: Rgb, secondary: Rgb, primary_weight: f32) -> Rgb {
    let w = primary_weight.clamp(0.0, 1.0);
    let inv = 1.0 - w;
    Rgb::new(
        ((primary.r as f32 * w) + (secondary.r as f32 * inv)).round() as u8,
        ((primary.g as f32 * w) + (secondary.g as f32 * inv)).round() as u8,
        ((primary.b as f32 * w) + (secondary.b as f32 * inv)).round() as u8,
    )
}

pub fn warm_shift(c: Rgb, amount: f32) -> Rgb {
    let t = amount.clamp(0.0, 1.0);
    let add = (t * 40.0).round() as u8;
    let sub = (t * 30.0).round() as u8;
    Rgb::new(c.r.saturating_add(add), c.g, c.b.saturating_sub(sub))
}

pub fn cool_shift(c: Rgb, amount: f32) -> Rgb {
    let amt = amount.clamp(0.0, 1.0);
    let r2 = (f32::from(c.r) * (1.0 - 0.5 * amt)).round() as u8;
    let b2 = (f32::from(c.b) + (255.0 - f32::from(c.b)) * 0.25 * amt).round() as u8;
    Rgb::new(r2, c.g, b2)
}

pub fn dim_shift(c: Rgb, amount: f32) -> Rgb {
    let m = 1.0 - amount.clamp(0.0, 1.0) * 0.5;
    Rgb::new(
        (c.r as f32 * m).round() as u8,
        (c.g as f32 * m).round() as u8,
        (c.b as f32 * m).round() as u8,
    )
}

pub fn tint_for_phase(c: Rgb, phase: DayPhase, blend: f32) -> Rgb {
    match phase {
        DayPhase::Day => c,
        DayPhase::Dawn => warm_shift(c, 0.10 * blend),
        DayPhase::Dusk => warm_shift(c, 0.18 * blend),
        DayPhase::Night => dim_shift(cool_shift(c, 0.18 * blend), 0.28 * blend),
    }
}

pub fn brighten_channel(c: Rgb, multiplier: f32) -> Rgb {
    let m = multiplier.max(0.0);
    Rgb::new(
        (c.r as f32 * m).min(255.0) as u8,
        (c.g as f32 * m).min(255.0) as u8,
        (c.b as f32 * m).min(255.0) as u8,
    )
}

pub fn darken_channel(c: Rgb, multiplier: f32) -> Rgb {
    let m = multiplier.clamp(0.0, 1.0);
    Rgb::new((c.r as f32 * m) as u8, (c.g as f32 * m) as u8, (c.b as f32 * m) as u8)
}

pub fn activity_lift_channel(c: Rgb, activity_level: f32) -> Rgb {
    let lift = (activity_level.clamp(0.0, 2.0) * 22.0) as u8;
    Rgb::new(c.r.saturating_add(lift), c.g.saturating_add(lift), c.b.saturating_add(lift))
}
```

Note the legacy split: `brighten_style`/`darken_style` use `.min(255.0) as u8` (truncating) and `as u8` (truncating) respectively, while `warm_shift`/`dim_shift`/`blend` use `.round()`. Preserve each exactly as written above — these rounding modes are load-bearing for byte-stable goldens.

Declare the module in `src/presentation/mod.rs`: `pub mod color_ops;`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib presentation::color_ops`
Expected: PASS (6 tests).

- [ ] **Step 5: Delegate the colors.rs wrappers (behavior-preserving)**

Rewrite the ratatui-level helpers in `src/tui/panels/pet/colors.rs` to call the new `color_ops` on the extracted `Rgb`, then re-wrap into `Color::Rgb`. Example for `warm_shift` (apply the same pattern to `cool_shift`, `dim_shift`, `blend_colors`, and the `match` inside `tint_style_for_phase`):

```rust
pub(super) fn warm_shift(base: Color, amount: f32) -> Color {
    let Color::Rgb(r, g, b) = base else { return base };
    let out = crate::presentation::color_ops::warm_shift(crate::pet::palette::Rgb::new(r, g, b), amount);
    Color::Rgb(out.r, out.g, out.b)
}
```

- [ ] **Step 6: Run the full suite + lints (goldens must be byte-stable)**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS — existing `colors.rs` tests and `dev-preview` goldens unchanged.

- [ ] **Step 7: Commit**

```bash
git add src/presentation/color_ops.rs src/presentation/mod.rs src/tui/panels/pet/colors.rs
git commit -m "refactor: extract neutral per-channel color ops into presentation::color_ops"
```

---

### Task 2: SurfaceStyle, LiveColorInputs, ResolvedColors types

Define the policy + inputs + output types. No logic yet.

**Files:**
- Create: `src/presentation/surface.rs`
- Modify: `src/presentation/mod.rs`
- Test: in `src/presentation/surface.rs`

**Interfaces:**
- Produces:
  - `pub enum EyeEmphasis { None, TerminalBold, Brightness }`
  - `pub struct SurfaceStyle { detail, clip, source_accent: bool, phase_tint: bool, energy_droop: bool, shimmer: bool, activity_lift: bool, prop_reaction: bool, eye_emphasis: EyeEmphasis }` (plus `privacy` — out of scope for this plan; omit the field for now, it lands with the scene work).
  - `pub const WATCH_STYLE / ROUND_STYLE / SCREEN_STYLE / MENU_STYLE: SurfaceStyle`
  - `pub struct LiveColorInputs { phase: DayPhase, phase_blend: f32, droop_mult: f32, shimmer_role: Option<PaletteRoleName>, shimmer_mult: f32, activity_level: f32, source_override: Option<Rgb> }`
  - `pub struct ResolvedColors { pub body, eye, mouth, accent, pattern, particle, corruption: Rgb, pub eye_emphasis: EyeEmphasis }`
- Consumes: `crate::pet::render::PaletteRoleName`, `crate::pet::palette::Rgb`, `crate::tui::day::DayPhase`.

(`detail: Detail` and `clip: Clip` are declared minimally here — `pub enum Detail { Full, Compact, Minimal }`, `pub enum Clip { None, Circle }` — but are unused until the scene/adapter plans; they exist so the constants are complete.)

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_style_enables_the_full_live_chain() {
        assert!(WATCH_STYLE.phase_tint && WATCH_STYLE.energy_droop && WATCH_STYLE.shimmer && WATCH_STYLE.activity_lift);
        assert!(!WATCH_STYLE.source_accent);
        assert!(matches!(WATCH_STYLE.eye_emphasis, EyeEmphasis::TerminalBold));
    }

    #[test]
    fn menu_style_is_source_accent_and_droop_only() {
        assert!(MENU_STYLE.source_accent);
        assert!(MENU_STYLE.energy_droop);
        assert!(!MENU_STYLE.phase_tint && !MENU_STYLE.shimmer && !MENU_STYLE.activity_lift);
        assert!(matches!(MENU_STYLE.eye_emphasis, EyeEmphasis::None));
    }

    #[test]
    fn round_style_today_is_passthrough_color() {
        // Companion currently applies no color transforms (it will gain them in the scene plan).
        assert!(!ROUND_STYLE.phase_tint && !ROUND_STYLE.energy_droop && !ROUND_STYLE.shimmer
                && !ROUND_STYLE.activity_lift && !ROUND_STYLE.source_accent);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib presentation::surface`
Expected: FAIL — types/constants not defined.

- [ ] **Step 3: Write minimal implementation**

Define the enums, structs, and the four constants in `src/presentation/surface.rs` per the Interfaces block. Concrete constant values:

```rust
pub const WATCH_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Full, clip: Clip::None,
    source_accent: false, phase_tint: true, energy_droop: true,
    shimmer: true, activity_lift: true, prop_reaction: true,
    eye_emphasis: EyeEmphasis::TerminalBold,
};
pub const ROUND_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Compact, clip: Clip::Circle,
    source_accent: false, phase_tint: false, energy_droop: false,
    shimmer: false, activity_lift: false, prop_reaction: false,
    eye_emphasis: EyeEmphasis::None,
};
pub const SCREEN_STYLE: SurfaceStyle = SurfaceStyle { detail: Detail::Full, clip: Clip::None, eye_emphasis: EyeEmphasis::Brightness, ..ROUND_STYLE };
pub const MENU_STYLE: SurfaceStyle = SurfaceStyle {
    detail: Detail::Minimal, clip: Clip::None,
    source_accent: true, phase_tint: false, energy_droop: true,
    shimmer: false, activity_lift: false, prop_reaction: false,
    eye_emphasis: EyeEmphasis::None,
};
```

(If `..ROUND_STYLE` struct-update in a `const` is rejected by the compiler, spell `SCREEN_STYLE` out in full — match `ROUND_STYLE` but `detail: Full`, `clip: None`, `eye_emphasis: Brightness`.)

Declare `pub mod surface;` and re-export the types in `src/presentation/mod.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib presentation::surface`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/presentation/surface.rs src/presentation/mod.rs
git commit -m "feat: add SurfaceStyle policy, LiveColorInputs, ResolvedColors types"
```

---

### Task 3: The unified resolver `resolve_pet_colors`

Compose the channel ops in the exact current watch order, each step gated by a `SurfaceStyle` knob.

**Files:**
- Modify: `src/presentation/surface.rs`
- Test: in `src/presentation/surface.rs`

**Interfaces:**
- Produces: `pub fn resolve_pet_colors(base: &ResolvedPalette, inputs: &LiveColorInputs, style: &SurfaceStyle) -> ResolvedColors`
- Consumes: `color_ops::*`, `crate::pet::palette::{ResolvedPalette, role_color, Rgb}`, `PaletteRoleName`.

- [ ] **Step 1: Write the failing tests**

Hand-derived expected values, computed from the current code:

```rust
#[cfg(test)]
mod resolver_tests {
    use super::*;
    use crate::pet::palette::{default_theme_palette, role_color, Rgb};
    use crate::pet::render::PaletteRoleName;
    use crate::tui::day::DayPhase;

    fn neutral_inputs() -> LiveColorInputs {
        LiveColorInputs {
            phase: DayPhase::Day, phase_blend: 0.0, droop_mult: 1.0,
            shimmer_role: None, shimmer_mult: 1.0, activity_level: 0.0, source_override: None,
        }
    }

    #[test]
    fn round_style_is_pure_role_color() {
        let p = default_theme_palette();
        let out = resolve_pet_colors(&p, &neutral_inputs(), &ROUND_STYLE);
        // companion does exactly role_color today
        assert_eq!(out.body, role_color(PaletteRoleName::Body, &p));
        assert_eq!(out.accent, role_color(PaletteRoleName::Accent, &p));
        assert!(matches!(out.eye_emphasis, EyeEmphasis::None));
    }

    #[test]
    fn menu_style_applies_source_override_then_sleep_dim() {
        // MENU_STYLE: source_accent on accent/particle, droop_mult carries the sleep dim.
        let p = default_theme_palette();
        let mut inputs = neutral_inputs();
        inputs.source_override = Some(Rgb::new(0xf0, 0xc4, 0x6a)); // Ensemble
        inputs.droop_mult = 0.7; // asleep
        let out = resolve_pet_colors(&p, &inputs, &MENU_STYLE);
        // accent = override(0xf0,0xc4,0x6a) darkened x0.7 (truncating) = (168,137,74)
        assert_eq!(out.accent, Rgb::new(168, 137, 74));
        // body = base body darkened x0.7, no override
        let body = role_color(PaletteRoleName::Body, &p);
        assert_eq!(out.body, Rgb::new((body.r as f32 * 0.7) as u8, (body.g as f32 * 0.7) as u8, (body.b as f32 * 0.7) as u8));
    }

    #[test]
    fn watch_style_runs_phase_then_droop_then_shimmer_then_lift_in_order() {
        let p = default_theme_palette();
        let mut inputs = neutral_inputs();
        inputs.phase = DayPhase::Dusk; inputs.phase_blend = 1.0;
        inputs.droop_mult = 0.8;
        inputs.shimmer_role = Some(PaletteRoleName::Pattern); inputs.shimmer_mult = 1.4;
        inputs.activity_level = 1.0;
        let out = resolve_pet_colors(&p, &inputs, &WATCH_STYLE);

        // recompute pattern by hand in the same order
        let mut c = role_color(PaletteRoleName::Pattern, &p);
        c = crate::presentation::color_ops::tint_for_phase(c, DayPhase::Dusk, 1.0);
        c = crate::presentation::color_ops::darken_channel(c, 0.8);
        c = crate::presentation::color_ops::brighten_channel(c, 1.4); // shimmer hits Pattern
        c = crate::presentation::color_ops::activity_lift_channel(c, 1.0);
        assert_eq!(out.pattern, c);
        // body gets the same chain MINUS shimmer (shimmer only hits Pattern)
        let mut b = role_color(PaletteRoleName::Body, &p);
        b = crate::presentation::color_ops::tint_for_phase(b, DayPhase::Dusk, 1.0);
        b = crate::presentation::color_ops::darken_channel(b, 0.8);
        b = crate::presentation::color_ops::activity_lift_channel(b, 1.0);
        assert_eq!(out.body, b);
        assert!(matches!(out.eye_emphasis, EyeEmphasis::TerminalBold));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib presentation::surface::resolver_tests`
Expected: FAIL — `resolve_pet_colors` not defined.

- [ ] **Step 3: Write minimal implementation**

```rust
use crate::presentation::color_ops;
use crate::pet::palette::{role_color, ResolvedPalette, Rgb};
use crate::pet::render::PaletteRoleName;

pub fn resolve_pet_colors(
    base: &ResolvedPalette,
    inputs: &LiveColorInputs,
    style: &SurfaceStyle,
) -> ResolvedColors {
    let resolve_one = |role: PaletteRoleName| -> Rgb {
        let mut c = role_color(role, base);
        // 1. source-accent override (menubar): accent/particle only
        if style.source_accent
            && matches!(role, PaletteRoleName::Accent | PaletteRoleName::Particle)
        {
            if let Some(over) = inputs.source_override {
                c = over;
            }
        }
        // 2. phase tint (watch)
        if style.phase_tint {
            c = color_ops::tint_for_phase(c, inputs.phase, inputs.phase_blend);
        }
        // 3. energy/sleep droop (watch energy*perf; menubar asleep?0.7:1.0)
        if style.energy_droop {
            c = color_ops::darken_channel(c, inputs.droop_mult);
        }
        // 4. shimmer / token-pop brighten (watch): one role only
        if style.shimmer && inputs.shimmer_role == Some(role) {
            c = color_ops::brighten_channel(c, inputs.shimmer_mult);
        }
        // 5. activity lift (watch)
        if style.activity_lift {
            c = color_ops::activity_lift_channel(c, inputs.activity_level);
        }
        c
    };
    ResolvedColors {
        body: resolve_one(PaletteRoleName::Body),
        eye: resolve_one(PaletteRoleName::Eye),
        mouth: resolve_one(PaletteRoleName::Mouth),
        accent: resolve_one(PaletteRoleName::Accent),
        pattern: resolve_one(PaletteRoleName::Pattern),
        particle: resolve_one(PaletteRoleName::Particle),
        corruption: resolve_one(PaletteRoleName::Corruption),
        eye_emphasis: style.eye_emphasis,
    }
}
```

(`EyeEmphasis` must derive `Clone, Copy` so it can be read out of `style` by value.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib presentation::surface`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/presentation/surface.rs
git commit -m "feat: resolve_pet_colors — one role->color resolver gated by SurfaceStyle"
```

---

### Task 4: Route companion through the resolver

**Files:**
- Modify: `src/companion/app.rs:504-510`
- Test: in `src/companion/app.rs`

**Interfaces:**
- Consumes: `resolve_pet_colors`, `ROUND_STYLE`, `LiveColorInputs`.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn companion_role_color_matches_resolver_round_style() {
    use crate::pet::palette::{default_theme_palette, role_color};
    use crate::pet::render::PaletteRoleName;
    let p = default_theme_palette();
    let got = pet_role_color(PaletteRoleName::Accent, &p).unwrap();
    let rgb = role_color(PaletteRoleName::Accent, &p);
    assert_eq!(got, rgb_color(rgb.r, rgb.g, rgb.b)); // unchanged from today
}
```

- [ ] **Step 2: Run test to verify it fails**

It will fail to compile only if you change the signature; run after Step 3. First confirm current green: `cargo test -p glorp companion_role_color_matches_resolver_round_style` → FAIL (test references nothing new yet; if it passes pre-change, that is fine — it pins current behavior).

Run: `cargo test companion_role_color_matches_resolver_round_style`
Expected: PASS against the *current* implementation (this is the characterization lock).

- [ ] **Step 3: Reroute through the resolver**

```rust
fn pet_role_color(
    role: PaletteRoleName,
    palette: &crate::pet::palette::ResolvedPalette,
) -> Option<RoundColor> {
    let resolved = crate::presentation::surface::resolve_pet_colors(
        palette,
        &crate::presentation::surface::LiveColorInputs::passthrough(),
        &crate::presentation::surface::ROUND_STYLE,
    );
    let rgb = crate::presentation::surface::role_rgb(&resolved, role);
    Some(rgb_color(rgb.r, rgb.g, rgb.b))
}
```

Add two small helpers in `surface.rs`: `LiveColorInputs::passthrough()` (all-neutral: `phase: Day, blend 0, droop_mult 1.0, shimmer None, shimmer_mult 1.0, activity 0, source_override None`) and `pub fn role_rgb(c: &ResolvedColors, role: PaletteRoleName) -> Rgb`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test companion_role_color_matches_resolver_round_style && cargo test --features dev-preview --test dev_preview`
Expected: PASS — round goldens byte-identical.

- [ ] **Step 5: Commit**

```bash
git add src/companion/app.rs src/presentation/surface.rs
git commit -m "refactor: companion resolves pet color through the shared resolver"
```

---

### Task 5: Route menubar through the resolver

**Files:**
- Modify: `src/menubar/render.rs:83-109`
- Test: in `src/menubar/render.rs`

**Interfaces:**
- Consumes: `resolve_pet_colors`, `MENU_STYLE`, `LiveColorInputs`, `role_rgb`.

- [ ] **Step 1: Write the failing test (hand-derived expected values)**

```rust
#[test]
fn menubar_accent_uses_source_override_and_sleep_dim() {
    // Build the source override + droop the same way role_color_for_profile does,
    // then assert the rerouted function reproduces today's output exactly.
    use crate::pet::palette::default_theme_palette;
    use crate::pet::render::PaletteRoleName;
    let palette = default_theme_palette();
    // Ensemble + asleep: accent override (0xf0,0xc4,0x6a) x0.7 = (168,137,74)
    let out = menubar_resolve(PaletteRoleName::Accent, &palette,
        Some(crate::pet::palette::Rgb::new(0xf0, 0xc4, 0x6a)), true);
    assert_eq!(out, Rgb(168, 137, 74));
    // Body, awake: base body unchanged
    let body = crate::pet::palette::role_color(PaletteRoleName::Body, &palette);
    let out_body = menubar_resolve(PaletteRoleName::Body, &palette, None, false);
    assert_eq!(out_body, Rgb(body.r, body.g, body.b));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test menubar_accent_uses_source_override_and_sleep_dim`
Expected: FAIL — `menubar_resolve` not defined.

- [ ] **Step 3: Extract the pure decision + reroute**

Replace `role_color_for_profile` (which reads the vm) with a thin shim over a pure `menubar_resolve(role, palette, source_override, asleep) -> Rgb` that calls the resolver:

```rust
fn menubar_resolve(
    role: PaletteRoleName,
    palette: &crate::pet::palette::ResolvedPalette,
    source_override: Option<crate::pet::palette::Rgb>,
    asleep: bool,
) -> Rgb {
    let inputs = crate::presentation::surface::LiveColorInputs {
        source_override,
        droop_mult: if asleep { SLEEP_DIM } else { 1.0 },
        ..crate::presentation::surface::LiveColorInputs::passthrough()
    };
    let resolved = crate::presentation::surface::resolve_pet_colors(
        palette, &inputs, &crate::presentation::surface::MENU_STYLE);
    let rgb = crate::presentation::surface::role_rgb(&resolved, role);
    Rgb(rgb.r, rgb.g, rgb.b)
}

fn role_color_for_profile(role: PaletteRoleName, vm: &WatchViewModel) -> Rgb {
    let source_override = menubar_source_override(role, vm); // existing accent/particle decision, returns Option<Rgb>
    menubar_resolve(role, &vm.pet_palette, source_override, vm.day_context.asleep)
}
```

Extract the current `match` at `render.rs:85-99` into `menubar_source_override(role, vm) -> Option<crate::pet::palette::Rgb>` returning `None` for non-accent/particle roles and for the `None => base` arm (so the resolver leaves the base untouched). Convert the existing `Rgb(..)` literals to `crate::pet::palette::Rgb::new(..)`.

- [ ] **Step 4: Run test + goldens**

Run: `cargo test menubar_accent_uses_source_override_and_sleep_dim && cargo test`
Expected: PASS — no menubar render test regressions.

- [ ] **Step 5: Commit**

```bash
git add src/menubar/render.rs src/presentation/surface.rs
git commit -m "refactor: menubar resolves pet color through the shared resolver"
```

---

### Task 6: Route watch through the resolver

Replace the inline 5-step `SemanticStyles` pipeline in `render_pet_inside` with one resolver call mapped to `SemanticStyles`. This is the byte-stability crux — the `dev-preview` watch goldens are the proof.

**Files:**
- Modify: `src/tui/panels/pet.rs:402-447`
- Modify: `src/tui/panels/pet/colors.rs` (add `semantic_styles_from_resolved`)
- Test: existing `dev-preview` watch goldens.

**Interfaces:**
- Consumes: `resolve_pet_colors`, `WATCH_STYLE`, `LiveColorInputs`, `ResolvedColors`, `EyeEmphasis`.
- Produces: `pub(super) fn semantic_styles_from_resolved(base: &SemanticStyles, c: &ResolvedColors) -> SemanticStyles`.

- [ ] **Step 1: Add the mapping helper (with a unit test)**

In `colors.rs`, add a function that seeds a `SemanticStyles` from `ResolvedColors`, applying eye `BOLD` when `eye_emphasis == TerminalBold` (reproducing `seed_pet_palette` + the bold eye):

```rust
pub(super) fn semantic_styles_from_resolved(
    base: &SemanticStyles,
    c: &crate::presentation::surface::ResolvedColors,
) -> SemanticStyles {
    use crate::presentation::surface::EyeEmphasis;
    let with = |style: Style, rgb: crate::pet::palette::Rgb| style.fg(Color::Rgb(rgb.r, rgb.g, rgb.b));
    let mut s = base.clone();
    s.pet_body = with(s.pet_body, c.body);
    s.pet_eye = with(s.pet_eye, c.eye);
    if matches!(c.eye_emphasis, EyeEmphasis::TerminalBold) {
        s.pet_eye = s.pet_eye.add_modifier(Modifier::BOLD);
    }
    s.pet_mouth = with(s.pet_mouth, c.mouth);
    s.pet_accent = with(s.pet_accent, c.accent);
    s.pet_pattern = with(s.pet_pattern, c.pattern);
    s.pet_particle = with(s.pet_particle, c.particle);
    s
}
```

Test: seed from a known `ResolvedColors`, assert `pet_eye.fg` and that `BOLD` is present.

- [ ] **Step 2: Replace the inline pipeline**

In `render_pet_inside` (`pet.rs:402-447`), build `LiveColorInputs` from the same values the inline chain used, call the resolver, and map to `SemanticStyles`:

```rust
let energy_m = low_energy_lightness_multiplier(vm.energy);
let perf_m = performance_lightness_multiplier(pet_performance);
let species = vm.pet_render.generated_species;
let shimmer_role = compute_shimmer_role(species, now);
let token_pop = profile_token_pop(vm.last_feed_pulse_at, &vm.life_profile, color_capability, now);
let effective_shimmer_role = if token_pop.is_some() { Some(PaletteRoleName::Pattern) } else { shimmer_role };
let shimmer_m = if effective_shimmer_role.is_some() { 1.4 } else { 1.0 };
let activity_level = if vm.life_profile.calm_mode { 0.0 } else { vm.life_profile.activity_level };
let phase_blend = {
    let since = (now - vm.day_context.phase_started_at_utc).whole_seconds() as f32;
    (since / (crate::tui::day::PHASE_BLEND_MINUTES as f32 * 60.0)).clamp(0.0, 1.0)
};
// Activity lift is a no-op on Flat terminals today (activity_lift_style early-returns);
// gate the knob the same way.
let style = if matches!(color_capability, crate::tui::style::ColorCapability::Flat) {
    crate::presentation::surface::SurfaceStyle { activity_lift: false, ..crate::presentation::surface::WATCH_STYLE }
} else {
    crate::presentation::surface::WATCH_STYLE
};
let inputs = crate::presentation::surface::LiveColorInputs {
    phase: vm.day_context.day_phase,
    phase_blend,
    droop_mult: energy_m * perf_m,
    shimmer_role: effective_shimmer_role,
    shimmer_mult: shimmer_m,
    activity_level,
    source_override: None,
};
let resolved = crate::presentation::surface::resolve_pet_colors(&vm.pet_palette, &inputs, &style);
let live_styles = semantic_styles_from_resolved(&semantic_styles(), &resolved);
```

Delete the now-unused `seed_pet_palette` → `tint_pet_styles_for_phase` → `darken_pet_styles` → `brighten_pet_role` → `lift_pet_styles_for_activity` chain from this function. The speech bubble previously used `droop`; pass `&live_styles` to `render_speech_bubble` instead (the speech bubble was drawn from the dimmed-but-not-shimmered styles — verify the golden for a speech frame; if it shifts, compute a `droop`-only `ResolvedColors` with `shimmer:false, activity_lift:false` and seed a separate `SemanticStyles` for the bubble to preserve byte-stability).

- [ ] **Step 3: Regenerate goldens and diff — expect NO change**

Run: `cargo run -- dev-preview --scenario all --out target/glorp-preview-verify`
Then diff against committed goldens (the dev-preview test compares automatically):
Run: `cargo test --features dev-preview --test dev_preview`
Expected: PASS — watch `cells.json` byte-identical. If any frame differs, the resolver/mapping diverged from the legacy chain; reconcile before proceeding (do not re-bake).

- [ ] **Step 4: Full suite + lints**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/tui/panels/pet.rs src/tui/panels/pet/colors.rs
git commit -m "refactor: watch resolves pet color through the shared resolver"
```

---

### Task 7: Anti-drift parity test + dead-code sweep

Lock in the property that prevents the next divergence, and remove the now-dead per-surface color logic.

**Files:**
- Create: `tests/pet_color_resolver_parity.rs` (or colocate in `surface.rs`)
- Modify: `src/tui/panels/pet/colors.rs` (delete now-unused whole-`SemanticStyles` helpers if no longer referenced), `src/companion/app.rs`, `src/menubar/render.rs`

**Interfaces:**
- Consumes: `resolve_pet_colors`, the four `*_STYLE` constants.

- [ ] **Step 1: Write the parity test**

```rust
#[test]
fn surfaces_share_one_base_resolution_and_differ_only_by_declared_knobs() {
    use glorp::pet::palette::{default_theme_palette, role_color};
    use glorp::pet::render::PaletteRoleName;
    use glorp::presentation::surface::*;
    let p = default_theme_palette();
    let neutral = LiveColorInputs::passthrough();
    // With neutral inputs, every knob is a no-op, so ALL surfaces must equal raw role_color.
    for style in [WATCH_STYLE, ROUND_STYLE, SCREEN_STYLE, MENU_STYLE] {
        let out = resolve_pet_colors(&p, &neutral, &style);
        for role in [PaletteRoleName::Body, PaletteRoleName::Mouth, PaletteRoleName::Pattern] {
            assert_eq!(role_rgb(&out, role), role_color(role, &p),
                "neutral inputs must collapse every surface to the shared base for {role:?}");
        }
    }
}
```

- [ ] **Step 2: Run it**

Run: `cargo test surfaces_share_one_base_resolution`
Expected: PASS.

- [ ] **Step 3: Delete dead per-surface helpers**

Remove any now-unreferenced functions from `colors.rs` (e.g. `seed_pet_palette`, `tint_pet_styles_for_phase`, `darken_pet_styles`, `brighten_pet_role`, `lift_pet_styles_for_activity`, `palette_from_styles`) **only if** `cargo build` confirms they have no remaining callers. Keep `activity_glyph_*` and the prop-glyph helpers (those serve glyphs, not pet roles). Run `cargo build` after each deletion to confirm no caller remains.

- [ ] **Step 4: Full suite + lints (the dead-code gate)**

Run: `cargo test && cargo clippy --all-targets --all-features -- -D warnings && cargo fmt --check`
Expected: PASS — clippy's dead-code/unused gate confirms nothing dangles.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test: cross-surface color parity guard; remove dead per-surface color paths"
```

---

## Self-Review

**Spec coverage (Tracks 0–1):**
- Track 0 (characterization net): Tasks 4/5/6 lock each surface's current output (companion passthrough, menubar hand-derived, watch goldens) before/while rerouting. ✓
- Track 1 (one resolver + SurfaceStyle): Tasks 1–3 build `color_ops` + `SurfaceStyle` + `resolve_pet_colors`; Tasks 4–6 route all three surfaces; Task 7 adds the anti-drift parity test and removes the triple path. ✓
- Corruption→accent quirk: handled inside the watch shimmer-brighten path (Task 3 resolver hits `Pattern`/role brighten; the `Corruption→pet_accent` legacy remap only mattered in `brighten_pet_role`, which is now subsumed — confirm no Corruption-role glyph relies on it via the watch golden). ⚠ verify in Task 6 golden diff.
- Constraints: `render_pet`/`art.rs` untouched (color work only); `presentation/` imports `crate::tui::day::DayPhase` and `crate::pet::*` but NOT `tui::component::TargetPath` ✓; goldens byte-stable (Tasks 4/6) ✓.

**Placeholder scan:** No TBD/TODO; every code step shows code; expected values hand-derived from current source.

**Type consistency:** `LiveColorInputs`, `ResolvedColors`, `role_rgb`, `passthrough`, `resolve_pet_colors`, the four `*_STYLE` constants, `EyeEmphasis`, `semantic_styles_from_resolved` are used consistently across Tasks 2–7.

**Known follow-up (out of scope, next plan):** `SurfaceStyle.privacy`, `detail`/`clip` consumption, `PetScene`, `SceneDrawList`, and effects-as-data land in Plan 02+. `LiveColorInputs` for companion/menubar are `passthrough` here because those surfaces apply no live chain yet; the companion gains the real chain when it moves onto `PetScene` (Plan 03).
