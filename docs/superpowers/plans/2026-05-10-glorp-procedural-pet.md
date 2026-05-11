# Glorp Procedural Pet (Phase 0 + Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the authored pet-art template system (`src/pet/art.rs` + slot-substitution rendering) with procedural Unicode generation rendered through Braille bitmap composition, so every pet has a structurally unique silhouette and a unique seeded evolution trajectory.

**Architecture:** A new `src/pet/generate.rs` owns the procedural pipeline — `PetBlueprint` (silhouette params + mutation vector + feature glyph picks) is derived deterministically from each pet's existing seed. The silhouette is sampled as a 2D bitmap (Gaussian envelope × head-zone gain × corner taper × seeded coherent noise, mirrored for bilateral symmetry, ornaments overlaid as a separate layer, eye-anchor cells reserved). The bitmap is encoded as Braille glyphs (2×4 px per char) with non-Braille feature glyphs overlaid at semantic anchors. Per-cell palette roles are assigned by region. `src/pet/render.rs` keeps its public surface (`render_pet`, `RenderedPet`, `StyledSegment`, etc.) and delegates internals to `generate.rs`. Phase 0 ships the generator as a standalone `examples/pet_gallery.rs` to iterate on aesthetic constants before promoting into the production module. Phase 1 promotes, deletes `art.rs`, and rewires `render.rs`.

**Tech Stack:** Rust 2021, ratatui 0.29 (unchanged in this plan), in-tree `StableRng` xorshift PRNG (no new deps), in-tree FNV-1a hashing, insta for snapshot tests (already a dev-dependency).

**Source spec:** `docs/superpowers/specs/2026-05-10-glorp-frontend-overhaul-design.md`

**This plan covers Phase 0 + Phase 1 only.** Phases 2 (layout overhaul), 3 (animation orchestration), 4 (tachyonfx transitions), 5 (mouse-tracked eyes), and 6 (polish) get separate plans after this one lands.

**Deliberate deviation from spec:** the spec's `PetBlueprint` struct includes `palette: PaletteRoles`, `feature_anchors: FeatureAnchors`, and `feature_glyphs: FeatureGlyphSet` fields alongside `silhouette` and `mutation_vector`. This plan keeps `PetBlueprint` minimal (species/stage/silhouette/mutation_vector); palette is owned by the existing `pet::generation::palette_roles(pet)` and feature anchors/glyphs are computed inside the rendering pipeline. Phase 3 (animator) will expand `PetBlueprint` to cache anchors and glyphs as fields so the animator can re-render cheaply across moods/ticks without re-deriving. Adding them now without an animator to consume them would be over-engineering.

---

## File map

**New:**
- `examples/pet_gallery.rs` — Phase 0 spike binary; ~200 LOC; prints 50 generated pets in a grid for human eyeball validation. Throwaway in the sense that Phase 1 promotes its core code into `pet/generate.rs`; the example file itself is retained as an ongoing dev tool.
- `src/pet/generate.rs` — production home for `PetBlueprint`, `SilhouetteParams`, `MutationVector`, the silhouette algorithm, ornament catalogue, braille encoder, feature glyph subsets, per-species biases, and per-cell palette role assignment. ~500 LOC after promotion.

**Modified:**
- `src/pet/render.rs` — keep public types (`RenderedPet`, `StyledSegment`, `PaletteRoleName`, `PaletteRoles`, `PaletteRole`, `AnimationFrame`, `AnimationProfile`) and public function `render_pet`. Replace internals: `render_pet` now derives a `PetBlueprint` via `pet::generate::blueprint_for` and renders it via `pet::generate::render_blueprint`. Delete `template_lines`-driven path. Shrinks from ~636 LOC → ~250 LOC.
- `src/pet/generation.rs` — promote `StableRng` from private to `pub(crate)`; add `next_f32_unit() -> f32` (0..1) and `next_bias(p: f32) -> bool` helpers; everything else unchanged.
- `src/pet/mod.rs` — remove `pub mod art;`, add `pub mod generate;`.

**Deleted:**
- `src/pet/art.rs` — ~630 LOC of authored templates and slot substitution helpers. All callers migrate to `pet::generate`.

**Test files:**
- `src/pet/generate.rs` — unit tests in `#[cfg(test)]` mod at the bottom of the file. Deterministic generator output gets insta snapshot tests against fixed seeds.
- `src/pet/render.rs` — existing tests update; new tests for the rewired `render_pet` against fixed seeds.

---

## Phase 0 — Procedural pet spike (gate)

### Task 1: Create the spike binary skeleton

**Files:**
- Create: `examples/pet_gallery.rs`

Cargo auto-detects files in `examples/` as runnable examples; no `Cargo.toml` change needed.

- [ ] **Step 1: Create the skeleton**

```rust
// examples/pet_gallery.rs
//
// Phase 0 spike: prints 50 procedurally-generated pets in a grid so the
// aesthetic-validation gate (zero broken pets, >=85% read as intentional
// creatures) can be checked by visual inspection.
//
// This file is standalone — no glorp internals — until Phase 1 promotes the
// generator into src/pet/generate.rs.

fn main() {
    println!("pet_gallery — Phase 0 procedural pet spike");
}
```

- [ ] **Step 2: Run it**

Run: `cargo run --example pet_gallery`
Expected: `pet_gallery — Phase 0 procedural pet spike` printed.

- [ ] **Step 3: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): scaffold pet_gallery spike binary"
```

### Task 2: Define core types — `SilhouetteParams`, `PetBlueprint`, `MutationVector`

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add types and a test for default-construction**

Append below `main`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilhouetteParams {
    pub width_px: u8,         // even — Braille is 2 wide per cell
    pub height_px: u8,        // multiple of 4 — Braille is 4 tall per cell
    pub roundness: f32,       // 0..1 — Gaussian envelope tightness
    pub taper: f32,           // 0..1 — corner falloff strength
    pub body_density: f32,    // 0..1 — overall fill probability
    pub asymmetry_seed: u32,
    pub head_zone_ratio: f32, // 0..1 — fraction of top grid reserved as head
    pub ornament_density: f32,
}

// Mirror of production `crate::game::evolution::Stage` so the spike has no
// glorp dependency. Production has S0..=S6; Phase 1 promotes to the real enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage { S0, S1, S2, S3, S4, S5, S6 }

// Mirror of production `crate::pet::generation::Species`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpeciesK {
    Fuzz, Blob, Ghost, Glitch, Crystal, Mech,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MutationVector {
    pub d_roundness: f32,
    pub d_taper: f32,
    pub d_body_density: f32,
    pub d_ornament_density: f32,
    pub d_head_zone_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct PetBlueprint {
    pub species: SpeciesK,
    pub stage: Stage,
    pub silhouette: SilhouetteParams,
    pub mutation_vector: MutationVector,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silhouette_params_round_trips() {
        let p = SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.5, taper: 0.5,
            body_density: 0.5, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        };
        assert_eq!(p, p.clone());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --example pet_gallery`
Expected: 1 test passes.

- [ ] **Step 3: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): core types for procedural generator"
```

### Task 3: Local seeded RNG with float and bias helpers

The spike must be deterministic per seed. We replicate `StableRng` locally to keep the spike free of glorp imports (Phase 1 will swap to the in-tree version).

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add RNG + tests for determinism**

```rust
#[derive(Debug, Clone)]
pub struct SpikeRng { state: u64 }

impl SpikeRng {
    pub fn new(seed: u64) -> Self { Self { state: seed.max(1) } }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform float in [0.0, 1.0).
    pub fn next_f32_unit(&mut self) -> f32 {
        // Use top 24 bits — float mantissa is 23+1 bits.
        ((self.next_u64() >> 40) as f32) / (1u32 << 24) as f32
    }

    /// Coin flip biased toward `p` (0..1).
    pub fn next_bias(&mut self, p: f32) -> bool { self.next_f32_unit() < p }

    /// Uniform i32 in [-1, 1] mapped from a float; for noise perturbation.
    pub fn next_signed_unit(&mut self) -> f32 { self.next_f32_unit() * 2.0 - 1.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ... keep silhouette_params_round_trips ...

    #[test]
    fn rng_is_deterministic_per_seed() {
        let mut a = SpikeRng::new(42);
        let mut b = SpikeRng::new(42);
        for _ in 0..1000 { assert_eq!(a.next_u64(), b.next_u64()); }
    }

    #[test]
    fn rng_f32_unit_is_within_bounds() {
        let mut r = SpikeRng::new(7);
        for _ in 0..10_000 {
            let f = r.next_f32_unit();
            assert!((0.0..1.0).contains(&f), "out of bounds: {f}");
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --example pet_gallery`
Expected: 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): seeded RNG for spike"
```

### Task 4: Aesthetic constants

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add the AESTHETIC block and a smoke test**

```rust
/// Starting values for spike tuning. Per spec, expected to shift during Phase 0.
pub mod aesthetic {
    pub const MIN_ROUNDNESS: f32 = 0.45;
    pub const MAX_TAPER: f32 = 0.75;
    pub const HEAD_ZONE_MIN_RATIO: f32 = 0.30;
    pub const MIN_FILLED_PIXELS_RATIO: f32 = 0.35;
    pub const MAX_ORNAMENT_DENSITY: [f32; 3] = [0.10, 0.25, 0.45]; // s0, s1, s2
    pub const EYE_ANCHOR_W_PX: u8 = 2;
    pub const EYE_ANCHOR_H_PX: u8 = 4;
    pub const RESAMPLE_RETRY_CAP: u8 = 6;
}

#[cfg(test)]
mod aesthetic_tests {
    use super::aesthetic::*;
    #[test]
    fn aesthetic_constants_in_unit_range() {
        for v in [MIN_ROUNDNESS, MAX_TAPER, HEAD_ZONE_MIN_RATIO, MIN_FILLED_PIXELS_RATIO] {
            assert!((0.0..=1.0).contains(&v), "{v} out of range");
        }
        for v in MAX_ORNAMENT_DENSITY { assert!((0.0..=1.0).contains(&v)); }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --example pet_gallery`
Expected: tests pass.

- [ ] **Step 3: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): aesthetic tuning constants"
```

### Task 5: Stage-to-grid-size mapping

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `grid_size(Stage)` and tests**

```rust
/// Pixel dimensions of the full bitmap for each stage.
///
/// The spec defines S0/S1/S2 (14×8, 18×12, 22×16). Production has S3..=S6 too.
/// For Phase 1 we cap S3+ at the S2 grid; richer geometry for late stages is
/// future work (tracked outside this plan).
pub fn stage_grid_full(stage: Stage) -> (u8, u8) {
    match stage {
        Stage::S0 => (14, 8),
        Stage::S1 => (18, 12),
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => (22, 16),
    }
}

#[cfg(test)]
mod stage_tests {
    use super::*;
    #[test]
    fn full_widths_are_even_and_heights_multiple_of_4() {
        for s in [Stage::S0, Stage::S1, Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            let (w, h) = stage_grid_full(s);
            assert_eq!(w % 2, 0, "width not even for {:?}: {w}", s);
            assert_eq!(h % 4, 0, "height not /4 for {:?}: {h}", s);
        }
    }
}
```

- [ ] **Step 2: Run tests; commit**

Run: `cargo test --example pet_gallery`
Then commit:

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): stage grid sizing"
```

### Task 6: Gaussian envelope fill probability

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `gaussian_envelope` + tests**

```rust
/// Returns a fill probability in [0, 1] for pixel (x, y) under a 2D Gaussian
/// envelope centered on (cx, cy) with sigma scaled by `roundness` and the
/// grid's larger dimension.
pub fn gaussian_envelope(
    x: f32, y: f32, cx: f32, cy: f32, half_w: f32, h: f32, roundness: f32,
) -> f32 {
    let sigma = (half_w.max(h) * 0.5) * (1.0 - roundness * 0.5).max(0.15);
    let dx = (x - cx) / sigma;
    let dy = (y - cy) / sigma;
    (-(dx * dx + dy * dy) * 0.5).exp().clamp(0.0, 1.0)
}

#[cfg(test)]
mod envelope_tests {
    use super::*;
    #[test]
    fn envelope_peaks_at_center() {
        let p_center = gaussian_envelope(3.5, 4.0, 3.5, 4.0, 7.0, 8.0, 0.5);
        let p_corner = gaussian_envelope(0.0, 0.0, 3.5, 4.0, 7.0, 8.0, 0.5);
        assert!(p_center > 0.95, "center: {p_center}");
        assert!(p_corner < p_center, "corner ({p_corner}) >= center ({p_center})");
    }

    #[test]
    fn higher_roundness_yields_tighter_envelope() {
        let off = (5.0, 6.0);
        let loose = gaussian_envelope(off.0, off.1, 3.5, 4.0, 7.0, 8.0, 0.20);
        let tight = gaussian_envelope(off.0, off.1, 3.5, 4.0, 7.0, 8.0, 0.95);
        assert!(loose > tight, "loose {loose} should be > tight {tight}");
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): gaussian envelope for silhouette fill probability"
```

### Task 7: Head-zone gain

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `head_zone_gain` + tests**

```rust
/// Returns a multiplier that boosts fill probability for cells in the top
/// `head_zone_ratio` of the grid. Always >= 1.0.
pub fn head_zone_gain(y: f32, h: f32, head_zone_ratio: f32) -> f32 {
    let cutoff = h * head_zone_ratio;
    if y < cutoff {
        let t = 1.0 - (y / cutoff).clamp(0.0, 1.0);
        1.0 + t * 0.35
    } else { 1.0 }
}

#[cfg(test)]
mod head_zone_tests {
    use super::*;
    #[test]
    fn head_zone_boosts_top() {
        let top = head_zone_gain(0.0, 8.0, 0.30);
        let middle = head_zone_gain(4.0, 8.0, 0.30);
        assert!(top > middle);
        assert!((middle - 1.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): head-zone gain"
```

### Task 8: Corner taper

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `corner_taper` + tests**

```rust
/// Returns a 0..1 multiplier that falls off toward the four corners of the
/// grid. `taper` (0..1) controls strength; higher = stronger falloff.
pub fn corner_taper(x: f32, y: f32, half_w: f32, h: f32, taper: f32) -> f32 {
    let nx = (x - half_w * 0.5).abs() / (half_w * 0.5).max(1.0);
    let ny = (y - h * 0.5).abs() / (h * 0.5).max(1.0);
    let radial = (nx * nx + ny * ny).sqrt().min(1.0);
    let strength = taper.clamp(0.0, 1.0);
    (1.0 - radial.powf(2.0) * strength).max(0.0)
}

#[cfg(test)]
mod taper_tests {
    use super::*;
    #[test]
    fn center_unaffected_corners_attenuated() {
        let center = corner_taper(3.5, 4.0, 7.0, 8.0, 0.75);
        let corner = corner_taper(0.0, 0.0, 7.0, 8.0, 0.75);
        assert!((center - 1.0).abs() < 1e-6);
        assert!(corner < 0.5);
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): corner taper"
```

### Task 9: Seeded coherent noise (single octave)

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `coherent_noise` + tests**

```rust
/// Cheap deterministic noise: hashes (x, y, seed) into a [-1, 1] perturbation.
/// Single octave is enough — we want subtle envelope distortion, not a fractal.
pub fn coherent_noise(x: i32, y: i32, seed: u32) -> f32 {
    // FNV-1a 32-bit on the (seed, x, y) triple
    let mut h: u32 = 0x811c_9dc5;
    for b in seed.to_le_bytes().iter()
        .chain(x.to_le_bytes().iter())
        .chain(y.to_le_bytes().iter())
    {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    ((h as f32) / (u32::MAX as f32)) * 2.0 - 1.0
}

#[cfg(test)]
mod noise_tests {
    use super::*;
    #[test]
    fn noise_is_deterministic() {
        assert_eq!(coherent_noise(3, 5, 42), coherent_noise(3, 5, 42));
    }
    #[test]
    fn noise_varies_across_coords() {
        let a = coherent_noise(0, 0, 7);
        let b = coherent_noise(1, 0, 7);
        let c = coherent_noise(0, 1, 7);
        assert!(a != b || a != c, "noise too flat");
    }
    #[test]
    fn noise_in_bounds() {
        for x in 0..30 { for y in 0..30 {
            let v = coherent_noise(x, y, 99);
            assert!((-1.0..=1.0).contains(&v), "out of bounds: {v}");
        }}
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): coherent noise for envelope perturbation"
```

### Task 10: Composite fill probability

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `fill_probability` + tests**

```rust
/// Combines envelope × head-zone × taper × (1 + noise×amplitude) into a
/// per-pixel fill probability.
pub fn fill_probability(
    x: i32, y: i32,
    params: &SilhouetteParams,
    noise_seed: u32,
) -> f32 {
    let half_w = (params.width_px / 2) as f32;
    let h = params.height_px as f32;
    let cx = half_w * 0.5;
    let cy = h * 0.5;

    let env = gaussian_envelope(x as f32, y as f32, cx, cy, half_w, h, params.roundness);
    let head = head_zone_gain(y as f32, h, params.head_zone_ratio.max(aesthetic::HEAD_ZONE_MIN_RATIO));
    let taper = corner_taper(x as f32, y as f32, half_w, h, params.taper.min(aesthetic::MAX_TAPER));
    let noise = coherent_noise(x, y, noise_seed) * 0.18; // amplitude tuned during spike

    (env * head * taper * (1.0 + noise)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod fill_tests {
    use super::*;
    fn default_params() -> SilhouetteParams {
        SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.55, taper: 0.55,
            body_density: 0.5, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        }
    }
    #[test]
    fn fill_probability_in_range() {
        let p = default_params();
        for x in 0..7 { for y in 0..8 {
            let f = fill_probability(x, y, &p, 1);
            assert!((0.0..=1.0).contains(&f));
        }}
    }
    #[test]
    fn center_more_likely_than_corner() {
        let p = default_params();
        let center = fill_probability(2, 3, &p, 1);
        let corner = fill_probability(0, 0, &p, 1);
        assert!(center > corner, "center {center} not > corner {corner}");
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): composite per-pixel fill probability"
```

### Task 11: Threshold, mirror, fill ratio rejection

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `sample_silhouette` + tests**

```rust
/// A boolean bitmap, indexed [y][x] with x in 0..width_px, y in 0..height_px.
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap { pub w: u8, pub h: u8, pub cells: Vec<bool> }

impl Bitmap {
    pub fn new(w: u8, h: u8) -> Self { Self { w, h, cells: vec![false; (w as usize) * (h as usize)] } }
    pub fn idx(&self, x: u8, y: u8) -> usize { (y as usize) * (self.w as usize) + (x as usize) }
    pub fn get(&self, x: u8, y: u8) -> bool { self.cells[self.idx(x, y)] }
    pub fn set(&mut self, x: u8, y: u8, v: bool) { let i = self.idx(x, y); self.cells[i] = v; }
    pub fn filled_ratio(&self) -> f32 {
        let on = self.cells.iter().filter(|&&b| b).count();
        on as f32 / self.cells.len() as f32
    }
}

/// Sample the half-grid, mirror to full width, return the bitmap. Retries up to
/// RESAMPLE_RETRY_CAP times with adjusted density if filled_ratio is below
/// MIN_FILLED_PIXELS_RATIO. Returns None if all retries are too sparse.
pub fn sample_silhouette(params: &SilhouetteParams, noise_seed: u32) -> Option<Bitmap> {
    let (full_w, h) = (params.width_px, params.height_px);
    let half_w = full_w / 2;
    let mut density = params.body_density;
    for _retry in 0..aesthetic::RESAMPLE_RETRY_CAP {
        let mut bm = Bitmap::new(full_w, h);
        for y in 0..h { for x in 0..half_w {
            let p = fill_probability(x as i32, y as i32, params, noise_seed);
            let on = p > (1.0 - density); // higher density = lower threshold
            bm.set(x, y, on);
            bm.set(full_w - 1 - x, y, on); // mirror
        }}
        if bm.filled_ratio() >= aesthetic::MIN_FILLED_PIXELS_RATIO { return Some(bm); }
        density = (density + 0.10).min(0.95); // bump and retry
    }
    None
}

#[cfg(test)]
mod silhouette_tests {
    use super::*;
    fn p() -> SilhouetteParams {
        SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.55, taper: 0.55,
            body_density: 0.55, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        }
    }
    #[test]
    fn silhouette_is_bilaterally_symmetric() {
        let bm = sample_silhouette(&p(), 1).expect("should produce silhouette");
        for y in 0..bm.h { for x in 0..(bm.w / 2) {
            assert_eq!(bm.get(x, y), bm.get(bm.w - 1 - x, y), "mismatch at ({x}, {y})");
        }}
    }
    #[test]
    fn silhouette_meets_min_fill_ratio() {
        let bm = sample_silhouette(&p(), 1).unwrap();
        assert!(bm.filled_ratio() >= aesthetic::MIN_FILLED_PIXELS_RATIO);
    }
    #[test]
    fn silhouette_is_deterministic_for_same_seed() {
        let a = sample_silhouette(&p(), 42).unwrap();
        let b = sample_silhouette(&p(), 42).unwrap();
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): bitmap silhouette sampling with mirror and retry"
```

### Task 12: Eye anchor reservation

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `EyeAnchors`, eye placement, body-exclusion enforcement, and tests**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeAnchors { pub left: (u8, u8), pub right: (u8, u8) }

/// Place two symmetric eye anchors inside the head zone, separated from the
/// vertical midline by a margin. Returns top-left pixel of each anchor cell.
pub fn place_eye_anchors(params: &SilhouetteParams) -> EyeAnchors {
    let full_w = params.width_px;
    let head_h = ((params.height_px as f32) * params.head_zone_ratio.max(aesthetic::HEAD_ZONE_MIN_RATIO)) as u8;
    let eye_y = (head_h / 2).saturating_sub(aesthetic::EYE_ANCHOR_H_PX / 2);
    let midline = full_w / 2;
    let margin = (full_w / 6).max(2);
    let left_x = midline.saturating_sub(margin + aesthetic::EYE_ANCHOR_W_PX);
    let right_x = midline + margin;
    EyeAnchors { left: (left_x, eye_y), right: (right_x, eye_y) }
}

/// After silhouette sampling, force eye-anchor cells off (no body pixels there).
pub fn reserve_eye_anchors(bm: &mut Bitmap, anchors: EyeAnchors) {
    for &(ax, ay) in &[anchors.left, anchors.right] {
        for dy in 0..aesthetic::EYE_ANCHOR_H_PX {
            for dx in 0..aesthetic::EYE_ANCHOR_W_PX {
                let x = ax.saturating_add(dx).min(bm.w - 1);
                let y = ay.saturating_add(dy).min(bm.h - 1);
                bm.set(x, y, false);
            }
        }
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::*;
    fn p() -> SilhouetteParams {
        SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.55, taper: 0.55,
            body_density: 0.55, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        }
    }
    #[test]
    fn eye_anchors_are_symmetric_around_midline() {
        let a = place_eye_anchors(&p());
        let midline = (p().width_px / 2) as i32;
        let left_d = midline - (a.left.0 as i32 + aesthetic::EYE_ANCHOR_W_PX as i32);
        let right_d = a.right.0 as i32 - midline;
        assert_eq!(left_d, right_d);
    }
    #[test]
    fn reservation_clears_anchor_cells() {
        let mut bm = sample_silhouette(&p(), 1).unwrap();
        let a = place_eye_anchors(&p());
        reserve_eye_anchors(&mut bm, a);
        for &(ax, ay) in &[a.left, a.right] {
            for dy in 0..aesthetic::EYE_ANCHOR_H_PX {
                for dx in 0..aesthetic::EYE_ANCHOR_W_PX {
                    let x = (ax + dx).min(bm.w - 1);
                    let y = (ay + dy).min(bm.h - 1);
                    assert!(!bm.get(x, y), "anchor cell ({x}, {y}) still filled");
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): eye anchor placement and reservation"
```

### Task 13: Ornament catalogue and symmetric placement

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add ornament pattern catalogue, symmetric placement, tests**

```rust
/// A small algorithmic pattern attached to the silhouette edge. Each pattern is
/// a list of (dx, dy) offsets relative to a placement anchor.
#[derive(Debug, Clone, Copy)]
pub enum OrnamentKind { Dot, Antenna, Fin, Hook }

pub fn ornament_pattern(kind: OrnamentKind) -> &'static [(i8, i8)] {
    match kind {
        OrnamentKind::Dot => &[(0, 0)],
        OrnamentKind::Antenna => &[(0, 0), (0, -1), (0, -2)],
        OrnamentKind::Fin => &[(0, 0), (1, 0), (1, 1)],
        OrnamentKind::Hook => &[(0, 0), (1, 0), (1, -1)],
    }
}

const SYM_ORNAMENT_KINDS: [OrnamentKind; 4] =
    [OrnamentKind::Dot, OrnamentKind::Antenna, OrnamentKind::Fin, OrnamentKind::Hook];

/// Apply 0..N symmetric ornament pairs along the silhouette top/side edges.
/// `n_pairs` is bounded by ornament_density × stage_max upstream.
pub fn add_symmetric_ornaments(
    bm: &mut Bitmap,
    rng: &mut SpikeRng,
    n_pairs: u8,
) {
    for _ in 0..n_pairs {
        let kind = SYM_ORNAMENT_KINDS[rng.next_usize_capped(SYM_ORNAMENT_KINDS.len())];
        // Pick a column near the silhouette edge in the top half
        let half_w = bm.w / 2;
        let col = rng.next_usize_capped(half_w as usize) as u8;
        let edge_y = find_top_filled(bm, col).unwrap_or(0);
        place_ornament(bm, kind, col, edge_y, false);
        place_ornament(bm, kind, bm.w - 1 - col, edge_y, true); // mirror with dx negated
    }
}

// `mirror=true` negates dx so asymmetric patterns (Fin, Hook) actually mirror
// instead of extending the same direction on both sides.
fn place_ornament(bm: &mut Bitmap, kind: OrnamentKind, ax: u8, ay: u8, mirror: bool) {
    for &(dx, dy) in ornament_pattern(kind) {
        let adx = if mirror { -dx } else { dx };
        let nx = (ax as i32 + adx as i32).clamp(0, bm.w as i32 - 1) as u8;
        let ny = (ay as i32 + dy as i32).clamp(0, bm.h as i32 - 1) as u8;
        bm.set(nx, ny, true);
    }
}

fn find_top_filled(bm: &Bitmap, x: u8) -> Option<u8> {
    (0..bm.h).find(|&y| bm.get(x, y))
}

impl SpikeRng {
    pub fn next_usize_capped(&mut self, upper: usize) -> usize {
        if upper == 0 { 0 } else { (self.next_u64() as usize) % upper }
    }
}

#[cfg(test)]
mod ornament_tests {
    use super::*;
    fn p() -> SilhouetteParams {
        SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.55, taper: 0.55,
            body_density: 0.55, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        }
    }
    #[test]
    fn symmetric_ornaments_preserve_symmetry() {
        let mut bm = sample_silhouette(&p(), 1).unwrap();
        let mut rng = SpikeRng::new(99);
        add_symmetric_ornaments(&mut bm, &mut rng, 3);
        for y in 0..bm.h { for x in 0..(bm.w / 2) {
            assert_eq!(bm.get(x, y), bm.get(bm.w - 1 - x, y));
        }}
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): symmetric ornament placement"
```

### Task 14: Bounded asymmetric ornaments

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add asymmetric ornament placement (0..=2 ornaments, single-side only) + tests**

```rust
/// Apply 0..=2 asymmetric ornaments to one side only, driven by asymmetry_seed.
/// Bounded count keeps the pet from becoming a chaotic protrusion field.
pub fn add_asymmetric_ornaments(bm: &mut Bitmap, asymmetry_seed: u32) {
    let mut rng = SpikeRng::new(asymmetry_seed as u64);
    let count = rng.next_usize_capped(3) as u8; // 0, 1, or 2
    let half_w = bm.w / 2;
    for _ in 0..count {
        let kind = SYM_ORNAMENT_KINDS[rng.next_usize_capped(SYM_ORNAMENT_KINDS.len())];
        let side_left = rng.next_bias(0.5);
        let col_in_half = rng.next_usize_capped(half_w as usize) as u8;
        let col = if side_left { col_in_half } else { bm.w - 1 - col_in_half };
        let edge_y = find_top_filled(bm, col).unwrap_or(0);
        // mirror=true on the right side so the ornament protrudes outward.
        place_ornament(bm, kind, col, edge_y, !side_left);
    }
}

#[cfg(test)]
mod async_tests {
    use super::*;
    fn p(seed: u32) -> SilhouetteParams {
        SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.55, taper: 0.55,
            body_density: 0.55, asymmetry_seed: seed,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        }
    }
    #[test]
    fn asymmetric_ornaments_deterministic_per_seed() {
        let mut a = sample_silhouette(&p(7), 1).unwrap();
        let mut b = sample_silhouette(&p(7), 1).unwrap();
        add_asymmetric_ornaments(&mut a, 7);
        add_asymmetric_ornaments(&mut b, 7);
        assert_eq!(a, b);
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): bounded asymmetric ornaments"
```

### Task 15: Braille encoder (bitmap → braille glyph)

**Files:**
- Modify: `examples/pet_gallery.rs`

Each Braille character covers 2 px wide × 4 px tall. The 8 dots map to a bit pattern, then offset by 0x2800.

- [ ] **Step 1: Add braille encoder + tests**

```rust
/// Convert a 2×4 pixel block (column-major) into the Unicode Braille glyph.
///
/// Bit mapping (per Unicode standard):
///   dx=0,dy=0 → 0x01    dx=1,dy=0 → 0x08
///   dx=0,dy=1 → 0x02    dx=1,dy=1 → 0x10
///   dx=0,dy=2 → 0x04    dx=1,dy=2 → 0x20
///   dx=0,dy=3 → 0x40    dx=1,dy=3 → 0x80
pub fn braille_block(bm: &Bitmap, x0: u8, y0: u8) -> char {
    let bit = |dx: u8, dy: u8, mask: u8| -> u8 {
        if bm.get(x0 + dx, y0 + dy) { mask } else { 0 }
    };
    let mut byte: u8 = 0;
    byte |= bit(0, 0, 0x01);
    byte |= bit(0, 1, 0x02);
    byte |= bit(0, 2, 0x04);
    byte |= bit(0, 3, 0x40);
    byte |= bit(1, 0, 0x08);
    byte |= bit(1, 1, 0x10);
    byte |= bit(1, 2, 0x20);
    byte |= bit(1, 3, 0x80);
    char::from_u32(0x2800 + byte as u32).unwrap()
}

/// Encode the full bitmap as braille lines. Each line is `width_px / 2` chars
/// wide. There are `height_px / 4` lines.
pub fn encode_braille(bm: &Bitmap) -> Vec<String> {
    let mut lines = Vec::with_capacity((bm.h / 4) as usize);
    for by in 0..(bm.h / 4) {
        let mut line = String::with_capacity((bm.w / 2) as usize);
        for bx in 0..(bm.w / 2) { line.push(braille_block(bm, bx * 2, by * 4)); }
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod braille_tests {
    use super::*;
    #[test]
    fn empty_block_is_blank_braille() {
        let bm = Bitmap::new(2, 4);
        assert_eq!(braille_block(&bm, 0, 0), '\u{2800}');
    }
    #[test]
    fn full_block_is_solid_braille() {
        let mut bm = Bitmap::new(2, 4);
        for y in 0..4 { for x in 0..2 { bm.set(x, y, true); }}
        assert_eq!(braille_block(&bm, 0, 0), '\u{28FF}');
    }
    #[test]
    fn encode_produces_expected_dims() {
        let mut bm = Bitmap::new(14, 8);
        for x in 0..14 { bm.set(x, 0, true); }
        let lines = encode_braille(&bm);
        assert_eq!(lines.len(), 2);
        for l in &lines { assert_eq!(l.chars().count(), 7); }
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): braille bitmap encoder"
```

### Task 16: Feature glyph subsets per species/stage

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add data tables + tests**

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureGlyphPick {
    pub eye: &'static str,    // single-glyph string (may be multi-byte)
    pub mouth: &'static str,
    pub accent: &'static str,
}

/// Stage-appropriate eye glyph alphabets per species.
fn eyes_for(species: SpeciesK, stage: Stage) -> &'static [&'static str] {
    match (species, stage) {
        (SpeciesK::Blob, Stage::S0) => &["o", "•", "●"],
        (SpeciesK::Blob, Stage::S1) => &["o", "•", "●", "◉"],
        (SpeciesK::Blob, _)         => &["◉", "◎", "⬢", "◐"], // S2+
        (SpeciesK::Mech, _)         => &["◇", "◆", "▣", "◫", "□"],
        (SpeciesK::Ghost, _)        => &["·", "°", "ʘ", "◌"],
        (SpeciesK::Glitch, _)       => &["x", "#", "0", "▩", "▤"],
        (SpeciesK::Crystal, _)      => &["◇", "◊", "⬡", "◈"],
        (SpeciesK::Fuzz, _)         => &["^", "u", "*", "•"],
    }
}

fn mouths_for(species: SpeciesK, _stage: Stage) -> &'static [&'static str] {
    match species {
        SpeciesK::Mech       => &["═", "─", "▪"],
        SpeciesK::Ghost      => &["", "·", "○"],
        SpeciesK::Glitch     => &["~", "≈", "─"],
        SpeciesK::Crystal    => &["◇", "◊"],
        SpeciesK::Blob       => &["w", "v", "ω"],
        SpeciesK::Fuzz       => &["w", "ᴗ", "ᵕ"],
    }
}

fn accents_for(species: SpeciesK, _stage: Stage) -> &'static [&'static str] {
    match species {
        SpeciesK::Mech    => &["╿", "│", "┃"],
        SpeciesK::Glitch  => &["▤", "▦", "░"],
        SpeciesK::Crystal => &["◆", "✦"],
        _                 => &["·", "•"],
    }
}

pub fn pick_features(species: SpeciesK, stage: Stage, rng: &mut SpikeRng) -> FeatureGlyphPick {
    let eyes = eyes_for(species, stage);
    let mouths = mouths_for(species, stage);
    let accents = accents_for(species, stage);
    FeatureGlyphPick {
        eye: eyes[rng.next_usize_capped(eyes.len())],
        mouth: mouths[rng.next_usize_capped(mouths.len())],
        accent: accents[rng.next_usize_capped(accents.len())],
    }
}

#[cfg(test)]
mod feature_tests {
    use super::*;
    #[test]
    fn picks_are_deterministic_per_seed() {
        let mut a = SpikeRng::new(7);
        let mut b = SpikeRng::new(7);
        for sp in [SpeciesK::Blob, SpeciesK::Mech, SpeciesK::Ghost] {
            for st in [Stage::S0, Stage::S1, Stage::S2] {
                assert_eq!(pick_features(sp, st, &mut a), pick_features(sp, st, &mut b));
            }
        }
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): per-species per-stage feature glyph subsets"
```

### Task 17: Render lines (braille body + feature overlay)

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `render_lines` that overlays features onto braille + tests**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct CharGrid { pub lines: Vec<Vec<char>> }

impl CharGrid {
    pub fn from_braille(braille_lines: Vec<String>) -> Self {
        let lines = braille_lines.into_iter().map(|s| s.chars().collect()).collect();
        Self { lines }
    }
    pub fn put(&mut self, char_x: usize, char_y: usize, c: char) {
        if let Some(row) = self.lines.get_mut(char_y) {
            if char_x < row.len() { row[char_x] = c; }
        }
    }
    pub fn into_string_lines(self) -> Vec<String> {
        self.lines.into_iter().map(|row| row.iter().collect()).collect()
    }
}

/// Convert pixel anchor (top-left of a 2×4 anchor cell) to the character cell.
fn px_to_char_cell(px_x: u8, px_y: u8) -> (usize, usize) {
    ((px_x / 2) as usize, (px_y / 4) as usize)
}

/// Build the final glyph grid: braille body, with eye glyphs at anchors and a
/// mouth glyph just below the eye-line in the head zone.
pub fn render_lines(
    bm: &Bitmap,
    anchors: EyeAnchors,
    features: &FeatureGlyphPick,
) -> Vec<String> {
    let braille = encode_braille(bm);
    let mut grid = CharGrid::from_braille(braille);

    let (lx, ly) = px_to_char_cell(anchors.left.0, anchors.left.1);
    let (rx, ry) = px_to_char_cell(anchors.right.0, anchors.right.1);
    if let Some(c) = features.eye.chars().next() {
        grid.put(lx, ly, c);
        grid.put(rx, ry, c);
    }

    let mouth_char_y = ly + 1;
    let mouth_char_x = ((bm.w / 2) / 2) as usize;
    if let Some(c) = features.mouth.chars().next() {
        grid.put(mouth_char_x, mouth_char_y, c);
    }

    grid.into_string_lines()
}

#[cfg(test)]
mod render_tests {
    use super::*;
    fn p() -> SilhouetteParams {
        SilhouetteParams {
            width_px: 14, height_px: 8, roundness: 0.55, taper: 0.55,
            body_density: 0.55, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: 0.10,
        }
    }
    #[test]
    fn rendered_lines_have_expected_dimensions() {
        let mut bm = sample_silhouette(&p(), 1).unwrap();
        let anchors = place_eye_anchors(&p());
        reserve_eye_anchors(&mut bm, anchors);
        let features = FeatureGlyphPick { eye: "o", mouth: "w", accent: "·" };
        let lines = render_lines(&bm, anchors, &features);
        assert_eq!(lines.len(), (p().height_px / 4) as usize);
        for l in &lines { assert_eq!(l.chars().count(), (p().width_px / 2) as usize); }
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): render braille bitmap + feature glyph overlay"
```

### Task 18: Per-species silhouette biases

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `species_bias(SpeciesK) -> SilhouetteParams` baseline + tests**

```rust
pub fn species_baseline(species: SpeciesK, stage: Stage) -> SilhouetteParams {
    let (w, h) = stage_grid_full(stage);
    let stage_idx = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 2,
    };
    let max_ornament = aesthetic::MAX_ORNAMENT_DENSITY[stage_idx];
    match species {
        SpeciesK::Blob    => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.75, taper: 0.55,
            body_density: 0.62, asymmetry_seed: 0,
            head_zone_ratio: 0.36, ornament_density: max_ornament * 0.5,
        },
        SpeciesK::Fuzz    => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.62, taper: 0.65,
            body_density: 0.66, asymmetry_seed: 0,
            head_zone_ratio: 0.40, ornament_density: max_ornament * 0.8,
        },
        SpeciesK::Mech    => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.50, taper: 0.40,
            body_density: 0.58, asymmetry_seed: 0,
            head_zone_ratio: 0.32, ornament_density: max_ornament,
        },
        SpeciesK::Ghost   => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.55, taper: 0.70,
            body_density: 0.50, asymmetry_seed: 0,
            head_zone_ratio: 0.45, ornament_density: max_ornament * 0.3,
        },
        SpeciesK::Glitch  => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.50, taper: 0.55,
            body_density: 0.60, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: max_ornament,
        },
        SpeciesK::Crystal => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.48, taper: 0.45,
            body_density: 0.55, asymmetry_seed: 0,
            head_zone_ratio: 0.32, ornament_density: max_ornament * 0.6,
        },
    }
}

#[cfg(test)]
mod baseline_tests {
    use super::*;
    #[test]
    fn baselines_satisfy_aesthetic_floors() {
        for sp in [SpeciesK::Blob, SpeciesK::Fuzz, SpeciesK::Mech,
                   SpeciesK::Ghost, SpeciesK::Glitch, SpeciesK::Crystal] {
            for st in [Stage::S0, Stage::S1, Stage::S2] {
                let p = species_baseline(sp, st);
                assert!(p.roundness >= aesthetic::MIN_ROUNDNESS, "{sp:?} {st:?}");
                assert!(p.taper <= aesthetic::MAX_TAPER, "{sp:?} {st:?}");
                assert!(p.head_zone_ratio >= aesthetic::HEAD_ZONE_MIN_RATIO, "{sp:?} {st:?}");
            }
        }
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): per-species silhouette baselines"
```

### Task 19: Mutation vector and stage progression

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add MutationVector derivation, apply-at-stage-up, tests**

```rust
/// Derive a deterministic mutation vector from the pet's primary seed.
pub fn derive_mutation_vector(seed: u64) -> MutationVector {
    let mut rng = SpikeRng::new(seed ^ 0xa1b2_c3d4_e5f6_0708);
    MutationVector {
        d_roundness: rng.next_signed_unit() * 0.10,
        d_taper: rng.next_signed_unit() * 0.10,
        d_body_density: rng.next_signed_unit() * 0.08,
        d_ornament_density: rng.next_signed_unit() * 0.06,
        d_head_zone_ratio: rng.next_signed_unit() * 0.05,
    }
}

/// Apply the mutation vector once, clamping to aesthetic floors/ceilings.
pub fn apply_mutation(params: SilhouetteParams, v: &MutationVector) -> SilhouetteParams {
    SilhouetteParams {
        roundness: (params.roundness + v.d_roundness)
            .max(aesthetic::MIN_ROUNDNESS).min(0.95),
        taper: (params.taper + v.d_taper)
            .min(aesthetic::MAX_TAPER).max(0.20),
        body_density: (params.body_density + v.d_body_density).clamp(0.40, 0.85),
        ornament_density: (params.ornament_density + v.d_ornament_density).clamp(0.0, 0.50),
        head_zone_ratio: (params.head_zone_ratio + v.d_head_zone_ratio)
            .max(aesthetic::HEAD_ZONE_MIN_RATIO).min(0.50),
        ..params
    }
}

pub fn blueprint_for(species: SpeciesK, stage: Stage, seed: u64) -> PetBlueprint {
    let mv = derive_mutation_vector(seed);
    let mut p = species_baseline(species, stage);
    p.asymmetry_seed = (seed >> 16) as u32 ^ (seed as u32);
    // Apply the mutation vector once per stage past S0. S3+ cap at three
    // applications until later stages get richer geometry.
    let mutations = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 => 2,
        Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 3,
    };
    for _ in 0..mutations { p = apply_mutation(p, &mv); }
    PetBlueprint { species, stage, silhouette: p, mutation_vector: mv }
}

#[cfg(test)]
mod mutation_tests {
    use super::*;
    #[test]
    fn mutation_vectors_are_seed_stable() {
        assert_eq!(derive_mutation_vector(42), derive_mutation_vector(42));
    }
    #[test]
    fn s2_diverges_from_s0_along_seed_direction() {
        let s0 = blueprint_for(SpeciesK::Blob, Stage::S0, 42);
        let s2 = blueprint_for(SpeciesK::Blob, Stage::S2, 42);
        let drift = (s2.silhouette.roundness - s0.silhouette.roundness).abs()
                  + (s2.silhouette.taper - s0.silhouette.taper).abs();
        assert!(drift > 0.0, "s2 should differ from s0 for the same seed");
    }
    #[test]
    fn two_seeds_diverge_by_s2() {
        let a = blueprint_for(SpeciesK::Blob, Stage::S2, 42).silhouette;
        let b = blueprint_for(SpeciesK::Blob, Stage::S2, 99).silhouette;
        assert!((a.roundness - b.roundness).abs() + (a.taper - b.taper).abs() > 0.01);
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): mutation vector and stage progression"
```

### Task 20: Top-level `generate_pet_lines` pipeline function

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add pipeline and tests**

```rust
/// End-to-end: seed + species + stage → rendered glyph lines.
pub fn generate_pet_lines(species: SpeciesK, stage: Stage, seed: u64) -> Vec<String> {
    let blueprint = blueprint_for(species, stage, seed);
    let noise_seed = (seed.wrapping_mul(0xdead_beef)) as u32;

    let mut bm = sample_silhouette(&blueprint.silhouette, noise_seed)
        .expect("silhouette retries should produce a bitmap at spec densities");

    let anchors = place_eye_anchors(&blueprint.silhouette);
    reserve_eye_anchors(&mut bm, anchors);

    let mut rng = SpikeRng::new(seed);
    let stage_idx = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 2,
    };
    // Promote to u32 before multiplying; bm.w * bm.h overflows u8 at S2 (22*16 = 352).
    let max_pairs = (blueprint.silhouette.ornament_density
                     * aesthetic::MAX_ORNAMENT_DENSITY[stage_idx]
                     * (bm.w as u32 * bm.h as u32) as f32 / 12.0) as u8;
    add_symmetric_ornaments(&mut bm, &mut rng, max_pairs.min(3));
    add_asymmetric_ornaments(&mut bm, blueprint.silhouette.asymmetry_seed);

    let features = pick_features(species, stage, &mut rng);
    render_lines(&bm, anchors, &features)
}

#[cfg(test)]
mod pipeline_tests {
    use super::*;
    #[test]
    fn pipeline_is_deterministic_per_seed() {
        let a = generate_pet_lines(SpeciesK::Blob, Stage::S0, 42);
        let b = generate_pet_lines(SpeciesK::Blob, Stage::S0, 42);
        assert_eq!(a, b);
    }
    #[test]
    fn pipeline_produces_expected_dimensions() {
        let lines = generate_pet_lines(SpeciesK::Mech, Stage::S2, 99);
        let (w, h) = stage_grid_full(Stage::S2);
        assert_eq!(lines.len(), (h / 4) as usize);
        for l in &lines { assert_eq!(l.chars().count(), (w / 2) as usize); }
    }
    #[test]
    fn two_seeds_produce_different_outputs_for_same_species() {
        let a = generate_pet_lines(SpeciesK::Blob, Stage::S2, 42);
        let b = generate_pet_lines(SpeciesK::Blob, Stage::S2, 99);
        assert_ne!(a, b);
    }
}
```

- [ ] **Step 2: Run tests; commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): top-level procedural pet pipeline"
```

### Task 21: Gallery printer — 50 pets in a grid

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Replace `main` to print the grid**

```rust
fn main() {
    println!("pet_gallery — Phase 0 procedural pet spike\n");
    let species: [SpeciesK; 6] = [
        SpeciesK::Fuzz, SpeciesK::Blob, SpeciesK::Ghost,
        SpeciesK::Glitch, SpeciesK::Crystal, SpeciesK::Mech,
    ];
    let stages: [Stage; 3] = [Stage::S0, Stage::S1, Stage::S2];

    // 50 pets = 6 species × 3 stages × ~3 seeds, capped at 50.
    let mut count: u32 = 0;
    for sp in species {
        for st in stages {
            for seed_base in 0u64..3 {
                if count >= 50 { return; }
                let seed = (sp as u64).wrapping_mul(0x9e37)
                    ^ (st as u64).wrapping_mul(0x7c15)
                    ^ (seed_base * 137);
                println!("--- #{count:02}  {sp:?}  {st:?}  seed={seed} ---");
                for line in generate_pet_lines(sp, st, seed) {
                    println!("    {line}");
                }
                println!();
                count += 1;
            }
        }
    }
}
```

- [ ] **Step 2: Run the gallery**

Run: `cargo run --example pet_gallery`
Expected: 50 pet blocks printed, each showing species/stage/seed and a small braille body with feature glyph eyes/mouth.

- [ ] **Step 3: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): gallery printer for 50-pet aesthetic gate"
```

### Task 22: Iterate `aesthetic` constants and per-species baselines until gate passes

**This is the gate. It does not have a unit-test pass/fail; the deliverable is human-validated.**

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Run gallery and visually inspect**

Run: `cargo run --example pet_gallery | less -R`

Walk through all 50 pets. Tally:
- Broken pets (disconnected blobs, sub-minimum body area, missing eye anchors, sharp rectangles, doesn't read as a creature).
- Intentional pets (cute or characterful).
- Weird-but-plausible pets (odd but creature-shaped).

- [ ] **Step 2: Tune until both criteria pass**

Per spec gate:
- Criterion 1: zero broken pets. If broken pets appear, adjust `MIN_FILLED_PIXELS_RATIO`, `RESAMPLE_RETRY_CAP`, `MIN_ROUNDNESS`, `MAX_TAPER`, the per-species `species_baseline` values, or `coherent_noise` amplitude. Re-run.
- Criterion 2: ≥85% intentional. If only 60% pass, tune species baselines (roundness, head_zone_ratio) and the ornament pattern catalogue. Re-run.

Each tuning iteration: edit constants → `cargo run --example pet_gallery` → review.

- [ ] **Step 3: Gate decision**

Record outcome:
- **Pass:** Both criteria met. Proceed to Phase 1 (Task 23).
- **Partial pass (criterion 1 only):** Stop, halt this plan, and re-plan Phase 1 with the compositional-parts fallback (small authored part library + procedural composition). The fallback is not detailed in this plan and would be planned separately.

- [ ] **Step 4: Commit final tuned constants**

```bash
git add examples/pet_gallery.rs
git commit -m "tune(pet): aesthetic constants and species baselines for Phase 0 gate"
```

---

## Phase 1 — Promotion to production module + render rewire

### Task 23: Promote `StableRng` to `pub(crate)` and add helpers

**Files:**
- Modify: `src/pet/generation.rs:219-247`

- [ ] **Step 1: Write tests for new helpers**

Add at the bottom of `src/pet/generation.rs`:

```rust
#[cfg(test)]
mod stable_rng_tests {
    use super::*;
    #[test]
    fn next_f32_unit_in_range() {
        let mut rng = StableRng::new(7);
        for _ in 0..10_000 {
            let f = rng.next_f32_unit();
            assert!((0.0..1.0).contains(&f), "out of bounds: {f}");
        }
    }
    #[test]
    fn next_bias_obeys_distribution() {
        let mut rng = StableRng::new(13);
        let n = 10_000;
        let hits = (0..n).filter(|_| rng.next_bias(0.30)).count() as f32 / n as f32;
        assert!((0.27..0.33).contains(&hits), "p=0.30 should land near 0.30, got {hits}");
    }
    #[test]
    fn next_signed_unit_in_signed_range() {
        let mut rng = StableRng::new(21);
        for _ in 0..1_000 {
            let v = rng.next_signed_unit();
            assert!((-1.0..=1.0).contains(&v));
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib pet::generation::stable_rng_tests`
Expected: FAIL ("cannot find method `next_f32_unit`" etc.)

- [ ] **Step 3: Promote `StableRng` and add helpers**

Edit `src/pet/generation.rs:219`:

```rust
#[derive(Debug, Clone)]
pub(crate) struct StableRng {
    state: u64,
}

impl StableRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    pub(crate) fn next_usize(&mut self, upper: usize) -> usize {
        debug_assert!(upper > 0);
        (self.next_u64() as usize) % upper
    }

    pub(crate) fn next_f32_unit(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / (1u32 << 24) as f32
    }

    pub(crate) fn next_bias(&mut self, p: f32) -> bool { self.next_f32_unit() < p }

    pub(crate) fn next_signed_unit(&mut self) -> f32 { self.next_f32_unit() * 2.0 - 1.0 }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib pet::generation::stable_rng_tests`
Expected: 3 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/pet/generation.rs
git commit -m "feat(pet): promote StableRng with f32/bias/signed helpers"
```

### Task 24: Create `src/pet/generate.rs` by promoting spike code

**Files:**
- Create: `src/pet/generate.rs`
- Modify: `src/pet/mod.rs`

The spike code lives in `examples/pet_gallery.rs`. We copy the *generator* portion (everything except `main` and `SpikeRng`) into `src/pet/generate.rs`, replacing `SpikeRng` with the now-public `crate::pet::generation::StableRng` and replacing `SpeciesK` with the existing `Species` from `pet::generation`.

- [ ] **Step 1: Create `src/pet/generate.rs` with the promoted code**

```rust
//! Procedural pet generation: seed → silhouette → braille bitmap + feature glyphs.
//!
//! Phase 0 lived in `examples/pet_gallery.rs`; this is its promotion into the
//! production module. The example is retained as an ongoing aesthetic-tuning
//! tool.

use crate::game::evolution::Stage;
use crate::pet::generation::{Species, StableRng};

pub mod aesthetic {
    pub const MIN_ROUNDNESS: f32 = 0.45;
    pub const MAX_TAPER: f32 = 0.75;
    pub const HEAD_ZONE_MIN_RATIO: f32 = 0.30;
    pub const MIN_FILLED_PIXELS_RATIO: f32 = 0.35;
    pub const MAX_ORNAMENT_DENSITY: [f32; 3] = [0.10, 0.25, 0.45];
    pub const EYE_ANCHOR_W_PX: u8 = 2;
    pub const EYE_ANCHOR_H_PX: u8 = 4;
    pub const RESAMPLE_RETRY_CAP: u8 = 6;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SilhouetteParams {
    pub width_px: u8,
    pub height_px: u8,
    pub roundness: f32,
    pub taper: f32,
    pub body_density: f32,
    pub asymmetry_seed: u32,
    pub head_zone_ratio: f32,
    pub ornament_density: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MutationVector {
    pub d_roundness: f32,
    pub d_taper: f32,
    pub d_body_density: f32,
    pub d_ornament_density: f32,
    pub d_head_zone_ratio: f32,
}

#[derive(Debug, Clone)]
pub struct PetBlueprint {
    pub species: Species,
    pub stage: Stage,
    pub silhouette: SilhouetteParams,
    pub mutation_vector: MutationVector,
}

// === Grid sizing =========================================================

pub fn stage_grid_full(stage: Stage) -> (u8, u8) {
    // S3..=S6 cap at the S2 grid for Phase 1; richer late-stage geometry is
    // tracked outside this plan.
    match stage {
        Stage::S0 => (14, 8),
        Stage::S1 => (18, 12),
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => (22, 16),
    }
}

// === Fill probability ====================================================

pub fn gaussian_envelope(
    x: f32, y: f32, cx: f32, cy: f32, half_w: f32, h: f32, roundness: f32,
) -> f32 {
    let sigma = (half_w.max(h) * 0.5) * (1.0 - roundness * 0.5).max(0.15);
    let dx = (x - cx) / sigma;
    let dy = (y - cy) / sigma;
    (-(dx * dx + dy * dy) * 0.5).exp().clamp(0.0, 1.0)
}

pub fn head_zone_gain(y: f32, h: f32, head_zone_ratio: f32) -> f32 {
    let cutoff = h * head_zone_ratio;
    if y < cutoff {
        let t = 1.0 - (y / cutoff).clamp(0.0, 1.0);
        1.0 + t * 0.35
    } else { 1.0 }
}

pub fn corner_taper(x: f32, y: f32, half_w: f32, h: f32, taper: f32) -> f32 {
    let nx = (x - half_w * 0.5).abs() / (half_w * 0.5).max(1.0);
    let ny = (y - h * 0.5).abs() / (h * 0.5).max(1.0);
    let radial = (nx * nx + ny * ny).sqrt().min(1.0);
    let strength = taper.clamp(0.0, 1.0);
    (1.0 - radial.powf(2.0) * strength).max(0.0)
}

pub fn coherent_noise(x: i32, y: i32, seed: u32) -> f32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in seed.to_le_bytes().iter()
        .chain(x.to_le_bytes().iter())
        .chain(y.to_le_bytes().iter())
    {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    ((h as f32) / (u32::MAX as f32)) * 2.0 - 1.0
}

pub fn fill_probability(x: i32, y: i32, params: &SilhouetteParams, noise_seed: u32) -> f32 {
    let half_w = (params.width_px / 2) as f32;
    let h = params.height_px as f32;
    let cx = half_w * 0.5;
    let cy = h * 0.5;
    let env = gaussian_envelope(x as f32, y as f32, cx, cy, half_w, h, params.roundness);
    let head = head_zone_gain(y as f32, h, params.head_zone_ratio.max(aesthetic::HEAD_ZONE_MIN_RATIO));
    let taper = corner_taper(x as f32, y as f32, half_w, h, params.taper.min(aesthetic::MAX_TAPER));
    let noise = coherent_noise(x, y, noise_seed) * 0.18;
    (env * head * taper * (1.0 + noise)).clamp(0.0, 1.0)
}

// === Bitmap ==============================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap { pub w: u8, pub h: u8, pub cells: Vec<bool> }

impl Bitmap {
    pub fn new(w: u8, h: u8) -> Self {
        Self { w, h, cells: vec![false; (w as usize) * (h as usize)] }
    }
    fn idx(&self, x: u8, y: u8) -> usize { (y as usize) * (self.w as usize) + (x as usize) }
    pub fn get(&self, x: u8, y: u8) -> bool { self.cells[self.idx(x, y)] }
    pub fn set(&mut self, x: u8, y: u8, v: bool) { let i = self.idx(x, y); self.cells[i] = v; }
    pub fn filled_ratio(&self) -> f32 {
        let on = self.cells.iter().filter(|&&b| b).count();
        on as f32 / self.cells.len() as f32
    }
}

pub fn sample_silhouette(params: &SilhouetteParams, noise_seed: u32) -> Option<Bitmap> {
    let (full_w, h) = (params.width_px, params.height_px);
    let half_w = full_w / 2;
    let mut density = params.body_density;
    for _ in 0..aesthetic::RESAMPLE_RETRY_CAP {
        let mut bm = Bitmap::new(full_w, h);
        for y in 0..h { for x in 0..half_w {
            let p = fill_probability(x as i32, y as i32, params, noise_seed);
            let on = p > (1.0 - density);
            bm.set(x, y, on);
            bm.set(full_w - 1 - x, y, on);
        }}
        if bm.filled_ratio() >= aesthetic::MIN_FILLED_PIXELS_RATIO { return Some(bm); }
        density = (density + 0.10).min(0.95);
    }
    None
}

// === Eye anchors =========================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeAnchors { pub left: (u8, u8), pub right: (u8, u8) }

pub fn place_eye_anchors(params: &SilhouetteParams) -> EyeAnchors {
    let full_w = params.width_px;
    let head_h = ((params.height_px as f32) * params.head_zone_ratio.max(aesthetic::HEAD_ZONE_MIN_RATIO)) as u8;
    let eye_y = (head_h / 2).saturating_sub(aesthetic::EYE_ANCHOR_H_PX / 2);
    let midline = full_w / 2;
    let margin = (full_w / 6).max(2);
    let left_x = midline.saturating_sub(margin + aesthetic::EYE_ANCHOR_W_PX);
    let right_x = midline + margin;
    EyeAnchors { left: (left_x, eye_y), right: (right_x, eye_y) }
}

pub fn reserve_eye_anchors(bm: &mut Bitmap, anchors: EyeAnchors) {
    for &(ax, ay) in &[anchors.left, anchors.right] {
        for dy in 0..aesthetic::EYE_ANCHOR_H_PX {
            for dx in 0..aesthetic::EYE_ANCHOR_W_PX {
                let x = ax.saturating_add(dx).min(bm.w - 1);
                let y = ay.saturating_add(dy).min(bm.h - 1);
                bm.set(x, y, false);
            }
        }
    }
}

// === Ornaments ===========================================================

#[derive(Debug, Clone, Copy)]
pub enum OrnamentKind { Dot, Antenna, Fin, Hook }

pub fn ornament_pattern(kind: OrnamentKind) -> &'static [(i8, i8)] {
    match kind {
        OrnamentKind::Dot => &[(0, 0)],
        OrnamentKind::Antenna => &[(0, 0), (0, -1), (0, -2)],
        OrnamentKind::Fin => &[(0, 0), (1, 0), (1, 1)],
        OrnamentKind::Hook => &[(0, 0), (1, 0), (1, -1)],
    }
}

const SYM_ORNAMENT_KINDS: [OrnamentKind; 4] =
    [OrnamentKind::Dot, OrnamentKind::Antenna, OrnamentKind::Fin, OrnamentKind::Hook];

// `mirror=true` negates dx so asymmetric patterns actually mirror.
fn place_ornament(bm: &mut Bitmap, kind: OrnamentKind, ax: u8, ay: u8, mirror: bool) {
    for &(dx, dy) in ornament_pattern(kind) {
        let adx = if mirror { -dx } else { dx };
        let nx = (ax as i32 + adx as i32).clamp(0, bm.w as i32 - 1) as u8;
        let ny = (ay as i32 + dy as i32).clamp(0, bm.h as i32 - 1) as u8;
        bm.set(nx, ny, true);
    }
}

fn find_top_filled(bm: &Bitmap, x: u8) -> Option<u8> {
    (0..bm.h).find(|&y| bm.get(x, y))
}

pub fn add_symmetric_ornaments(bm: &mut Bitmap, rng: &mut StableRng, n_pairs: u8) {
    for _ in 0..n_pairs {
        let kind = SYM_ORNAMENT_KINDS[rng.next_usize(SYM_ORNAMENT_KINDS.len())];
        let half_w = bm.w / 2;
        let col = rng.next_usize(half_w as usize) as u8;
        let edge_y = find_top_filled(bm, col).unwrap_or(0);
        place_ornament(bm, kind, col, edge_y, false);
        place_ornament(bm, kind, bm.w - 1 - col, edge_y, true);
    }
}

pub fn add_asymmetric_ornaments(bm: &mut Bitmap, asymmetry_seed: u32) {
    let mut rng = StableRng::new(asymmetry_seed as u64);
    let count = rng.next_usize(3) as u8;
    let half_w = bm.w / 2;
    for _ in 0..count {
        let kind = SYM_ORNAMENT_KINDS[rng.next_usize(SYM_ORNAMENT_KINDS.len())];
        let side_left = rng.next_bias(0.5);
        let col_in_half = rng.next_usize(half_w as usize) as u8;
        let col = if side_left { col_in_half } else { bm.w - 1 - col_in_half };
        let edge_y = find_top_filled(bm, col).unwrap_or(0);
        place_ornament(bm, kind, col, edge_y, !side_left);
    }
}

// === Braille encoder =====================================================

pub fn braille_block(bm: &Bitmap, x0: u8, y0: u8) -> char {
    let bit = |dx: u8, dy: u8, mask: u8| -> u8 {
        if bm.get(x0 + dx, y0 + dy) { mask } else { 0 }
    };
    let mut byte: u8 = 0;
    byte |= bit(0, 0, 0x01);
    byte |= bit(0, 1, 0x02);
    byte |= bit(0, 2, 0x04);
    byte |= bit(0, 3, 0x40);
    byte |= bit(1, 0, 0x08);
    byte |= bit(1, 1, 0x10);
    byte |= bit(1, 2, 0x20);
    byte |= bit(1, 3, 0x80);
    char::from_u32(0x2800 + byte as u32).unwrap()
}

pub fn encode_braille(bm: &Bitmap) -> Vec<String> {
    let mut lines = Vec::with_capacity((bm.h / 4) as usize);
    for by in 0..(bm.h / 4) {
        let mut line = String::with_capacity((bm.w / 2) as usize);
        for bx in 0..(bm.w / 2) { line.push(braille_block(bm, bx * 2, by * 4)); }
        lines.push(line);
    }
    lines
}

// === Feature glyphs ======================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureGlyphPick {
    pub eye: &'static str,
    pub mouth: &'static str,
    pub accent: &'static str,
}

fn eyes_for(species: Species, stage: Stage) -> &'static [&'static str] {
    match (species, stage) {
        (Species::Blob, Stage::S0) => &["o", "•", "●"],
        (Species::Blob, Stage::S1) => &["o", "•", "●", "◉"],
        (Species::Blob, _)         => &["◉", "◎", "⬢", "◐"], // S2+
        (Species::Mech, _)         => &["◇", "◆", "▣", "◫", "□"],
        (Species::Ghost, _)        => &["·", "°", "ʘ", "◌"],
        (Species::Glitch, _)       => &["x", "#", "0", "▩", "▤"],
        (Species::Crystal, _)      => &["◇", "◊", "⬡", "◈"],
        (Species::Fuzz, _)         => &["^", "u", "*", "•"],
    }
}

fn mouths_for(species: Species, _stage: Stage) -> &'static [&'static str] {
    match species {
        Species::Mech    => &["═", "─", "▪"],
        Species::Ghost   => &["·", "○", "─"],
        Species::Glitch  => &["~", "≈", "─"],
        Species::Crystal => &["◇", "◊"],
        Species::Blob    => &["w", "v", "ω"],
        Species::Fuzz    => &["w", "ᴗ", "ᵕ"],
    }
}

fn accents_for(species: Species, _stage: Stage) -> &'static [&'static str] {
    match species {
        Species::Mech    => &["╿", "│", "┃"],
        Species::Glitch  => &["▤", "▦", "░"],
        Species::Crystal => &["◆", "✦"],
        _                => &["·", "•"],
    }
}

pub fn pick_features(species: Species, stage: Stage, rng: &mut StableRng) -> FeatureGlyphPick {
    let eyes = eyes_for(species, stage);
    let mouths = mouths_for(species, stage);
    let accents = accents_for(species, stage);
    FeatureGlyphPick {
        eye: eyes[rng.next_usize(eyes.len())],
        mouth: mouths[rng.next_usize(mouths.len())],
        accent: accents[rng.next_usize(accents.len())],
    }
}

// === Render lines ========================================================

pub fn render_lines(bm: &Bitmap, anchors: EyeAnchors, features: &FeatureGlyphPick) -> Vec<String> {
    let braille = encode_braille(bm);
    let mut grid: Vec<Vec<char>> = braille.into_iter().map(|s| s.chars().collect()).collect();
    let put = |g: &mut Vec<Vec<char>>, char_x: usize, char_y: usize, c: char| {
        if let Some(row) = g.get_mut(char_y) {
            if char_x < row.len() { row[char_x] = c; }
        }
    };
    let (lx, ly) = ((anchors.left.0 / 2) as usize, (anchors.left.1 / 4) as usize);
    let (rx, ry) = ((anchors.right.0 / 2) as usize, (anchors.right.1 / 4) as usize);
    if let Some(c) = features.eye.chars().next() {
        put(&mut grid, lx, ly, c);
        put(&mut grid, rx, ry, c);
    }
    let mouth_y = ly + 1;
    let mouth_x = ((bm.w / 2) / 2) as usize;
    if let Some(c) = features.mouth.chars().next() { put(&mut grid, mouth_x, mouth_y, c); }
    grid.into_iter().map(|r| r.into_iter().collect()).collect()
}

// === Per-species baselines ==============================================

pub fn species_baseline(species: Species, stage: Stage) -> SilhouetteParams {
    let (w, h) = stage_grid_full(stage);
    let stage_idx = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 2,
    };
    let max_ornament = aesthetic::MAX_ORNAMENT_DENSITY[stage_idx];
    match species {
        Species::Blob    => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.75, taper: 0.55,
            body_density: 0.62, asymmetry_seed: 0,
            head_zone_ratio: 0.36, ornament_density: max_ornament * 0.5,
        },
        Species::Fuzz    => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.62, taper: 0.65,
            body_density: 0.66, asymmetry_seed: 0,
            head_zone_ratio: 0.40, ornament_density: max_ornament * 0.8,
        },
        Species::Mech    => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.50, taper: 0.40,
            body_density: 0.58, asymmetry_seed: 0,
            head_zone_ratio: 0.32, ornament_density: max_ornament,
        },
        Species::Ghost   => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.55, taper: 0.70,
            body_density: 0.50, asymmetry_seed: 0,
            head_zone_ratio: 0.45, ornament_density: max_ornament * 0.3,
        },
        Species::Glitch  => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.50, taper: 0.55,
            body_density: 0.60, asymmetry_seed: 0,
            head_zone_ratio: 0.30, ornament_density: max_ornament,
        },
        Species::Crystal => SilhouetteParams {
            width_px: w, height_px: h, roundness: 0.48, taper: 0.45,
            body_density: 0.55, asymmetry_seed: 0,
            head_zone_ratio: 0.32, ornament_density: max_ornament * 0.6,
        },
    }
}

// === Mutation vector + blueprint =======================================

pub fn derive_mutation_vector(seed: u64) -> MutationVector {
    let mut rng = StableRng::new(seed ^ 0xa1b2_c3d4_e5f6_0708);
    MutationVector {
        d_roundness: rng.next_signed_unit() * 0.10,
        d_taper: rng.next_signed_unit() * 0.10,
        d_body_density: rng.next_signed_unit() * 0.08,
        d_ornament_density: rng.next_signed_unit() * 0.06,
        d_head_zone_ratio: rng.next_signed_unit() * 0.05,
    }
}

pub fn apply_mutation(p: SilhouetteParams, v: &MutationVector) -> SilhouetteParams {
    SilhouetteParams {
        roundness: (p.roundness + v.d_roundness).max(aesthetic::MIN_ROUNDNESS).min(0.95),
        taper: (p.taper + v.d_taper).min(aesthetic::MAX_TAPER).max(0.20),
        body_density: (p.body_density + v.d_body_density).clamp(0.40, 0.85),
        ornament_density: (p.ornament_density + v.d_ornament_density).clamp(0.0, 0.50),
        head_zone_ratio: (p.head_zone_ratio + v.d_head_zone_ratio)
            .max(aesthetic::HEAD_ZONE_MIN_RATIO).min(0.50),
        ..p
    }
}

pub fn blueprint_for(species: Species, stage: Stage, seed: u64) -> PetBlueprint {
    let mv = derive_mutation_vector(seed);
    let mut p = species_baseline(species, stage);
    p.asymmetry_seed = (seed >> 16) as u32 ^ (seed as u32);
    let mutations = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 => 2,
        Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 3,
    };
    for _ in 0..mutations { p = apply_mutation(p, &mv); }
    PetBlueprint { species, stage, silhouette: p, mutation_vector: mv }
}

// === Top-level pipeline ================================================

pub fn generate_pet_lines(species: Species, stage: Stage, seed: u64) -> Vec<String> {
    let blueprint = blueprint_for(species, stage, seed);
    let noise_seed = (seed.wrapping_mul(0xdead_beef)) as u32;
    let mut bm = sample_silhouette(&blueprint.silhouette, noise_seed)
        .expect("silhouette retries should produce a bitmap at spec densities");
    let anchors = place_eye_anchors(&blueprint.silhouette);
    reserve_eye_anchors(&mut bm, anchors);

    let mut rng = StableRng::new(seed);
    let stage_idx = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 2,
    };
    // Promote to u32 before multiplying; bm.w * bm.h overflows u8 at S2 (22*16 = 352).
    let max_pairs = (blueprint.silhouette.ornament_density
                     * aesthetic::MAX_ORNAMENT_DENSITY[stage_idx]
                     * (bm.w as u32 * bm.h as u32) as f32 / 12.0) as u8;
    add_symmetric_ornaments(&mut bm, &mut rng, max_pairs.min(3));
    add_asymmetric_ornaments(&mut bm, blueprint.silhouette.asymmetry_seed);

    let features = pick_features(species, stage, &mut rng);
    render_lines(&bm, anchors, &features)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_deterministic_per_seed() {
        let a = generate_pet_lines(Species::Blob, Stage::S0, 42);
        let b = generate_pet_lines(Species::Blob, Stage::S0, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn pipeline_dimensions_match_stage() {
        let lines = generate_pet_lines(Species::Mech, Stage::S2, 99);
        let (w, h) = stage_grid_full(Stage::S2);
        assert_eq!(lines.len(), (h / 4) as usize);
        for l in &lines { assert_eq!(l.chars().count(), (w / 2) as usize); }
    }

    #[test]
    fn two_seeds_diverge() {
        let a = generate_pet_lines(Species::Blob, Stage::S2, 42);
        let b = generate_pet_lines(Species::Blob, Stage::S2, 99);
        assert_ne!(a, b);
    }

    #[test]
    fn mutation_vector_is_seed_stable() {
        assert_eq!(derive_mutation_vector(42), derive_mutation_vector(42));
    }

    #[test]
    fn baselines_satisfy_aesthetic_floors() {
        for sp in [Species::Blob, Species::Fuzz, Species::Mech,
                   Species::Ghost, Species::Glitch, Species::Crystal] {
            for st in [Stage::S0, Stage::S1, Stage::S2] {
                let p = species_baseline(sp, st);
                assert!(p.roundness >= aesthetic::MIN_ROUNDNESS, "{sp:?} {st:?}");
                assert!(p.head_zone_ratio >= aesthetic::HEAD_ZONE_MIN_RATIO, "{sp:?} {st:?}");
            }
        }
    }
}
```

- [ ] **Step 2: Register the module in `src/pet/mod.rs`**

Replace the file contents:

```rust
pub mod art;
pub mod generate;
pub mod generation;
pub mod render;
```

(Keep `art` for now — Task 27 deletes it after Task 26 migrates render.)

- [ ] **Step 3: Build**

Run: `cargo build --lib`
Expected: compiles cleanly.

- [ ] **Step 4: Run tests**

Run: `cargo test --lib pet::generate::tests`
Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/pet/generate.rs src/pet/mod.rs
git commit -m "feat(pet): promote procedural generator to src/pet/generate.rs"
```

### Task 25: Switch `examples/pet_gallery.rs` to import from `crate`

The spike file is now redundant with `src/pet/generate.rs`. Reduce it to a thin wrapper that exercises the production generator — so the spike binary stays useful as an aesthetic-tuning tool.

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Replace the file**

```rust
//! Aesthetic-tuning binary. Prints 50 procedurally-generated pets in a grid.
//! Uses the production generator at glorp::pet::generate.

use glorp::game::evolution::Stage;
use glorp::pet::generate::generate_pet_lines;
use glorp::pet::generation::Species;

fn main() {
    println!("pet_gallery — procedural pet aesthetic gate\n");
    let species = [
        Species::Fuzz, Species::Blob, Species::Ghost,
        Species::Glitch, Species::Crystal, Species::Mech,
    ];
    let stages: [Stage; 3] = [Stage::S0, Stage::S1, Stage::S2];

    let mut count = 0u32;
    for sp in species {
        for st in stages {
            for seed_base in 0u64..3 {
                if count >= 50 { return; }
                let seed = (sp as u64).wrapping_mul(0x9e37)
                    ^ (st as u64).wrapping_mul(0x7c15)
                    ^ (seed_base * 137);
                println!("--- #{count:02}  {sp:?}  {st:?}  seed={seed} ---");
                for line in generate_pet_lines(sp, st, seed) {
                    println!("    {line}");
                }
                println!();
                count += 1;
            }
        }
    }
}
```

`src/lib.rs` already exists and exposes `pub mod pet` and `pub mod game`, so this import works without any structural change.

- [ ] **Step 2: Run the gallery against the production generator**

Run: `cargo run --example pet_gallery`
Expected: 50 pet blocks. Output should match the final tuned state from Task 22 (since the constants are copied verbatim).

- [ ] **Step 3: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): point pet_gallery example at production generator"
```

### Task 26: Rewrite `src/pet/render.rs` against `generate.rs`

`render.rs` currently consumes `pet::art::template_lines` and slot tokens. It must instead consume a `PetBlueprint` (via `generate_pet_lines`) and overlay blink/mood expression swaps on top.

**Files:**
- Modify: `src/pet/render.rs`

- [ ] **Step 1: Write failing tests for the new render path**

Add to `src/pet/render.rs` (append):

```rust
#[cfg(test)]
mod render_v2_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::generate_pet;

    #[test]
    fn render_pet_produces_braille_lines_for_blob_s0() {
        let pet = generate_pet("test-seed-1");
        let r = render_pet(&pet, Stage::S0, Mood::Content,
                           AnimationFrame { tick: 0, blink_suppression_ticks: 0 });
        assert!(!r.lines.is_empty());
        let has_braille = r.lines.iter().any(|l|
            l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))));
        assert!(has_braille, "expected at least one braille char in rendered lines");
    }

    #[test]
    fn render_pet_is_deterministic_for_fixed_seed_and_frame() {
        let pet = generate_pet("test-seed-2");
        let frame = AnimationFrame { tick: 5, blink_suppression_ticks: 0 };
        let a = render_pet(&pet, Stage::S1, Mood::Content, frame);
        let b = render_pet(&pet, Stage::S1, Mood::Content, frame);
        assert_eq!(a.lines, b.lines);
        assert_eq!(a.spans, b.spans);
    }

    #[test]
    fn render_pet_two_seeds_diverge() {
        let pet_a = generate_pet("seed-aaa");
        let pet_b = generate_pet("seed-bbb");
        let frame = AnimationFrame { tick: 0, blink_suppression_ticks: 0 };
        let a = render_pet(&pet_a, Stage::S2, Mood::Content, frame);
        let b = render_pet(&pet_b, Stage::S2, Mood::Content, frame);
        assert_ne!(a.lines, b.lines);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib pet::render::render_v2_tests`
Expected: FAIL (existing `render_pet` doesn't use generate.rs and probably doesn't emit braille).

- [ ] **Step 3: Rewrite `render_pet` to consume the procedural pipeline**

Replace the body of `render_pet` and the helpers it calls. The new implementation:

1. Derive a `u64` seed from the pet's `seed` string via FNV-1a (already in scope as `fnv1a64`).
2. Call `crate::pet::generate::generate_pet_lines(pet.species, stage, seed_u64)` to get the body lines.
3. Apply blink/mood expression substitution by replacing the eye chars at the known eye-character positions in the rendered lines, computed from `EyeAnchors` (call `place_eye_anchors` directly on the same blueprint).
4. Emit per-cell `StyledSegment`s by walking each output line: braille cells → `Body`/`Pattern`/`Accent` (by region), feature glyph cells → `Eye`/`Mouth`.

Replacement (top of `render.rs`, replacing the existing `render_pet`):

```rust
use crate::pet::generate::{
    blueprint_for, place_eye_anchors, sample_silhouette, reserve_eye_anchors,
    add_symmetric_ornaments, add_asymmetric_ornaments, pick_features,
    encode_braille, aesthetic,
};
use crate::pet::generation::{StableRng, fnv1a64, GeneratedPet, Species};

pub fn render_pet(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    frame: AnimationFrame,
) -> RenderedPet {
    let seed_u64 = fnv1a64(&pet.seed);
    let blueprint = blueprint_for(pet.species, stage, seed_u64);
    let noise_seed = seed_u64.wrapping_mul(0xdead_beef) as u32;

    let mut bm = sample_silhouette(&blueprint.silhouette, noise_seed)
        .expect("silhouette generation should succeed at spec densities");
    let anchors = place_eye_anchors(&blueprint.silhouette);
    reserve_eye_anchors(&mut bm, anchors);

    let mut rng = StableRng::new(seed_u64);
    let stage_idx = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => 2,
    };
    // Promote to u32 before multiplying; bm.w * bm.h overflows u8 at S2 (22*16 = 352).
    let max_pairs = (blueprint.silhouette.ornament_density
                     * aesthetic::MAX_ORNAMENT_DENSITY[stage_idx]
                     * (bm.w as u32 * bm.h as u32) as f32 / 12.0) as u8;
    add_symmetric_ornaments(&mut bm, &mut rng, max_pairs.min(3));
    add_asymmetric_ornaments(&mut bm, blueprint.silhouette.asymmetry_seed);

    let profile = species_animation_profile(pet.species);
    let blinking = should_blink(pet, mood, frame, profile);
    let features = pick_features(pet.species, stage, &mut rng);

    let lines = render_lines_with_expression(&bm, anchors, &features, mood, blinking, pet.species);
    let spans = build_spans(&lines, anchors, bm.w, bm.h);

    RenderedPet { lines, spans, event_lines: Vec::new() }
}

/// Render the bitmap as braille and overlay mood/blink-aware feature glyphs.
fn render_lines_with_expression(
    bm: &crate::pet::generate::Bitmap,
    anchors: crate::pet::generate::EyeAnchors,
    features: &crate::pet::generate::FeatureGlyphPick,
    mood: Mood,
    blinking: bool,
    species: Species,
) -> Vec<String> {
    let braille = encode_braille(bm);
    let mut grid: Vec<Vec<char>> = braille.into_iter().map(|s| s.chars().collect()).collect();

    let eye_char = if blinking {
        closed_blink_eyes(species).chars().next().unwrap_or('-')
    } else {
        match mood {
            Mood::Content => features.eye.chars().next().unwrap_or('o'),
            Mood::Sleepy  => '-',
            Mood::Hungry  => 'u',
            Mood::Sad     => '`',
            Mood::Happy   => '^',
            Mood::Wilted  => '.',
        }
    };
    let mouth_char = match mood {
        Mood::Sleepy => '~',
        Mood::Sad    => '_',
        Mood::Happy  => 'ω',
        Mood::Wilted => '.',
        _            => features.mouth.chars().next().unwrap_or('v'),
    };

    let put = |g: &mut Vec<Vec<char>>, char_x: usize, char_y: usize, c: char| {
        if let Some(row) = g.get_mut(char_y) {
            if char_x < row.len() { row[char_x] = c; }
        }
    };
    let (lx, ly) = ((anchors.left.0 / 2) as usize, (anchors.left.1 / 4) as usize);
    let (rx, ry) = ((anchors.right.0 / 2) as usize, (anchors.right.1 / 4) as usize);
    put(&mut grid, lx, ly, eye_char);
    put(&mut grid, rx, ry, eye_char);
    let mouth_y = ly + 1;
    let mouth_x = ((bm.w / 2) / 2) as usize;
    put(&mut grid, mouth_x, mouth_y, mouth_char);

    grid.into_iter().map(|r| r.into_iter().collect()).collect()
}

/// Per-cell role assignment. Eye cells → Eye, mouth → Mouth, body braille → Body.
fn build_spans(lines: &[String], anchors: crate::pet::generate::EyeAnchors, bm_w: u8, bm_h: u8)
    -> Vec<StyledSegment>
{
    let mut out = Vec::new();
    let char_w = (bm_w / 2) as usize;
    let (lx, ly) = ((anchors.left.0 / 2) as usize, (anchors.left.1 / 4) as usize);
    let (rx, ry) = ((anchors.right.0 / 2) as usize, (anchors.right.1 / 4) as usize);
    let mouth_y = ly + 1;
    let mouth_x = char_w / 2;
    for (line_idx, line) in lines.iter().enumerate() {
        for (ch_idx, c) in line.chars().enumerate() {
            let role = if line_idx == ly && (ch_idx == lx || ch_idx == rx)
                    || line_idx == ry && (ch_idx == lx || ch_idx == rx) {
                PaletteRoleName::Eye
            } else if line_idx == mouth_y && ch_idx == mouth_x {
                PaletteRoleName::Mouth
            } else if (0x2800..=0x28FF).contains(&(c as u32)) {
                PaletteRoleName::Body
            } else {
                PaletteRoleName::Accent
            };
            out.push(StyledSegment {
                line: line_idx, start: ch_idx, end: ch_idx + 1, role,
            });
        }
    }
    out
}
```

Also delete the now-unused `use crate::pet::art::*` import at the top of `render.rs`, and remove any helper functions in `render.rs` that referenced `template_lines`, `expression_for`, or `stage_key` if they're now unused (rustc will flag these).

- [ ] **Step 4: Make `fnv1a64` accessible from render**

`fnv1a64` was a private free function in `pet::generation`. Make it `pub(crate)`:

```rust
// in src/pet/generation.rs, change signature:
pub(crate) fn fnv1a64(s: &str) -> u64 { /* existing body */ }
```

- [ ] **Step 5: Build and test**

Run: `cargo build --lib`
Then: `cargo test --lib pet::render`
Expected: builds; new render_v2 tests pass. Existing render tests may fail because outputs changed — that's expected; update or replace them rather than gaming them to pass.

- [ ] **Step 6: Update existing render tests that asserted on the old template output**

Find any tests in `render.rs` that asserted specific characters or line widths corresponding to the old `art.rs` templates. Either:
- Delete them if they're now redundant with `render_v2_tests`, or
- Rewrite them to assert on the new braille-based output (use the dimensions from `stage_grid_full`).

Run: `cargo test --lib pet`
Expected: all pet-module tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/pet/render.rs src/pet/generation.rs
git commit -m "feat(pet): rewire render.rs to consume procedural generator"
```

### Task 27: Delete `src/pet/art.rs` and migrate stragglers

**Files:**
- Delete: `src/pet/art.rs`
- Modify: `src/pet/mod.rs`
- Modify (likely): anywhere else in the codebase that referenced `pet::art::*`

- [ ] **Step 1: Find remaining callers**

Run: `grep -rn "pet::art\|pet/art\|use crate::pet::art" src/ examples/ tests/ benches/`
Expected: only `src/pet/mod.rs` (the `pub mod art;` declaration) after Task 26 ran cleanly. If any other files reference `pet::art::*`, fix them — most likely candidates are display helpers or status-command renderers that pulled `template_lines` directly.

For each external caller, replace with the equivalent `pet::generate::generate_pet_lines` call. If the caller wanted the *static template* for a non-live use (e.g., generating preview art for `glorp status`), call `generate_pet_lines` with a tick-stable frame.

- [ ] **Step 2: Remove `pub mod art;` from `src/pet/mod.rs`**

```rust
pub mod generate;
pub mod generation;
pub mod render;
```

- [ ] **Step 3: Delete the file**

```bash
rm src/pet/art.rs
```

- [ ] **Step 4: Build the full project**

Run: `cargo build --all-targets`
Expected: builds. If anything fails to resolve `art::*`, fix it (return to Step 1).

- [ ] **Step 5: Run the full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add -u src/pet/ src/pet/mod.rs
# (and any external caller files you migrated)
git commit -m "feat(pet): delete art.rs; templates fully replaced by procedural generator"
```

### Task 28: End-to-end smoke test — `glorp watch`

The whole point of this overhaul is the live watch view rendering procedural pets. Verify it.

**Files:** none modified.

- [ ] **Step 1: Build the binary**

Run: `cargo build --bin glorp`
Expected: builds cleanly with no warnings about unused art-related items.

- [ ] **Step 2: Run watch mode against a test state**

If a test/dev glorp state already exists:

Run: `cargo run --bin glorp -- watch`

If not, initialize one first:

Run: `cargo run --bin glorp -- init`
Then: `cargo run --bin glorp -- watch`

Expected: the watch view renders, the pet panel shows braille glyphs with feature-glyph eyes/mouth. Press `q` to exit.

- [ ] **Step 3: Snapshot manual observations**

Visually verify:
- Pet renders in the pet column without clipping.
- Pet eyes blink occasionally.
- Mood expression (mood-driven mouth) renders for current mood.
- No panics or rendering glitches when terminal is resized.

If the pet looks broken in the live view despite passing the Phase 0 gate in isolation, the issue is likely in `build_spans` (palette role assignment producing wrong colors) or `render_lines_with_expression`'s anchor coordinate math. Debug from there.

- [ ] **Step 4: Run lints**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings. Fix any that surfaced from the deletion of `art.rs` and the rewrite.

- [ ] **Step 5: Commit if any clippy fixes were needed**

```bash
git add -u
git commit -m "chore: clippy fixes after procedural pet migration"
```

### Task 29: Insta snapshot tests for the generator

Lock in the tuned aesthetic so future changes don't silently regress it.

**Files:**
- Modify: `src/pet/generate.rs`

- [ ] **Step 1: Add snapshot tests using insta (already a dev-dependency)**

Append to the `tests` mod at the bottom of `src/pet/generate.rs`:

```rust
    #[test]
    fn snapshot_blob_s0() {
        let lines = generate_pet_lines(Species::Blob, Stage::S0, 42);
        insta::assert_snapshot!(lines.join("\n"));
    }

    #[test]
    fn snapshot_mech_s2() {
        let lines = generate_pet_lines(Species::Mech, Stage::S2, 99);
        insta::assert_snapshot!(lines.join("\n"));
    }

    #[test]
    fn snapshot_ghost_s1() {
        let lines = generate_pet_lines(Species::Ghost, Stage::S1, 137);
        insta::assert_snapshot!(lines.join("\n"));
    }
```

- [ ] **Step 2: Generate the initial snapshots**

Run: `cargo test --lib pet::generate::tests::snapshot_ -- --include-ignored`
Expected: 3 tests fail (no snapshots exist yet). Then:

Run: `cargo insta review`
Accept each snapshot (visually verify each looks like a creature first).

If `cargo-insta` is not installed:

Run: `cargo install cargo-insta`

- [ ] **Step 3: Re-run to confirm green**

Run: `cargo test --lib pet::generate::tests::snapshot_`
Expected: 3 tests pass.

- [ ] **Step 4: Commit snapshots**

```bash
git add src/pet/generate.rs src/pet/snapshots/
git commit -m "test(pet): lock procedural generator output via insta snapshots"
```

### Task 30: Final regression sweep

**Files:** none modified (unless fixes needed).

- [ ] **Step 1: Full test suite**

Run: `cargo test`
Expected: all tests pass, including layout/integration tests.

- [ ] **Step 2: Clippy clean**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 3: Format check**

Run: `cargo fmt --all -- --check`
Expected: clean. If not: `cargo fmt --all` and commit.

- [ ] **Step 4: Verify LOC delta against spec budget**

Run: `git diff --stat $(git log --oneline | grep "scaffold pet_gallery" | tail -1 | cut -d' ' -f1)^ HEAD -- src/pet/ examples/pet_gallery.rs`

Expected rough delta (per spec budget for Phase 0 + Phase 1): ~700 net LOC removed across the touched files. Confirm we're roughly in that range; substantial overshoot is a code-smell to investigate.

- [ ] **Step 5: Final commit if anything got cleaned up**

```bash
git add -u
git commit -m "chore: final cleanup after procedural pet migration"
```

---

## Summary

This plan delivers:
- A standalone `examples/pet_gallery.rs` for aesthetic tuning (Phase 0).
- The procedural generator promoted to `src/pet/generate.rs` (Phase 1).
- `src/pet/render.rs` rewired to consume the generator.
- `src/pet/art.rs` deleted.
- Insta snapshots locking the tuned output.

Remaining phases (2 layout overhaul, 3 animator, 4 tachyonfx, 5 mouse-tracked eyes, 6 polish) get their own plans after this one ships.
