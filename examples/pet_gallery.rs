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

#[derive(Debug, Clone)]
pub struct SpikeRng {
    state: u64,
}

impl SpikeRng {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Uniform float in [0.0, 1.0).
    pub fn next_f32_unit(&mut self) -> f32 {
        // Use top 24 bits — float mantissa is 23+1 bits.
        ((self.next_u64() >> 40) as f32) / (1u32 << 24) as f32
    }

    /// Coin flip biased toward `p` (0..1).
    pub fn next_bias(&mut self, p: f32) -> bool {
        self.next_f32_unit() < p
    }

    /// Uniform f32 in [-1.0, 1.0]; for noise perturbation.
    pub fn next_signed_unit(&mut self) -> f32 {
        self.next_f32_unit() * 2.0 - 1.0
    }
}

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

/// Returns a multiplier that boosts fill probability for cells in the top
/// `head_zone_ratio` of the grid. Always >= 1.0.
pub fn head_zone_gain(y: f32, h: f32, head_zone_ratio: f32) -> f32 {
    let cutoff = h * head_zone_ratio;
    if y < cutoff {
        let t = 1.0 - (y / cutoff).clamp(0.0, 1.0);
        1.0 + t * 0.35
    } else { 1.0 }
}

/// Returns a 0..1 multiplier that falls off toward the four corners of the
/// grid. `taper` (0..1) controls strength; higher = stronger falloff.
pub fn corner_taper(x: f32, y: f32, half_w: f32, h: f32, taper: f32) -> f32 {
    let nx = (x - half_w * 0.5).abs() / (half_w * 0.5).max(1.0);
    let ny = (y - h * 0.5).abs() / (h * 0.5).max(1.0);
    let radial = (nx * nx + ny * ny).sqrt().min(1.0);
    let strength = taper.clamp(0.0, 1.0);
    (1.0 - radial.powf(2.0) * strength).max(0.0)
}

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

    #[test]
    fn rng_is_deterministic_per_seed() {
        let mut a = SpikeRng::new(42);
        let mut b = SpikeRng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
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
