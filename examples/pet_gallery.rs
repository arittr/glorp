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
        place_ornament(bm, kind, bm.w - 1 - col, edge_y, true); // mirror
    }
}

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

impl SpikeRng {
    pub fn next_usize_capped(&mut self, upper: usize) -> usize {
        if upper == 0 { 0 } else { (self.next_u64() as usize) % upper }
    }
}

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
