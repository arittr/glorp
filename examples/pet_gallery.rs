// examples/pet_gallery.rs
//
// Phase 0 compositional-parts spike: scaffolding reset.
// The algorithmic Gaussian generator has been removed; gallery printer wired in v2 Task 8.
//
// This file is standalone — no glorp internals — until Phase 1 promotes the
// generator into src/pet/generate.rs.

fn main() {
    println!("parts_gallery — Blob proof-of-concept\n");
    for stage in [Stage::S0, Stage::S1, Stage::S2] {
        for seed in [42u64, 99, 137, 7, 21] {
            println!("--- Blob {stage:?} seed={seed} ---");
            for line in generate_pet_lines(SpeciesK::Blob, stage, seed) {
                println!("    {line}");
            }
            println!();
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSelection {
    pub head: PartId,
    pub body: PartId,
    pub accessories: Vec<PartId>, // 0..=N
}

#[derive(Debug, Clone)]
pub struct PetBlueprint {
    pub species: SpeciesK,
    pub stage: Stage,
    pub selection: PartSelection,
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

// ============================================================================
// Part types for compositional bitmap authoring.
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PartId(pub u16);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    HeadCenter,   // head part centered horizontally, top of grid
    BodyCenter,   // body part centered horizontally, below head zone
    HeadTop,      // accessory attaches to top edge of head part
    BodySide,     // accessory attaches to side of body part
    BodyBottom,   // accessory attaches to bottom of body part
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartSymmetry {
    Symmetric,      // pixels drawn as a whole symmetric shape
    HalfMirror,     // pixels are the left half; composer mirrors to right
    AsymmetricFree, // pixels are placed once, no mirror (e.g., single antenna)
}

/// A small authored bitmap pattern with metadata for composition.
/// Pixels are stored row-major in `rows`: each u32 represents one row, with
/// bit 0 (lsb) = column 0 of that row. `width_px` columns, `rows.len()` rows.
#[derive(Debug, Clone, Copy)]
pub struct Part {
    pub id: PartId,
    pub rows: &'static [u32],
    pub width_px: u8,
    pub height_px: u8,
    pub anchor: Anchor,
    pub min_stage: Stage,
    pub symmetry: PartSymmetry,
    pub eye_anchors: Option<EyeAnchors>, // present on head parts only
}

pub struct PartCatalog {
    pub heads: &'static [Part],
    pub bodies: &'static [Part],
    pub accessories: &'static [Part],
}

/// Render a single Part into the bitmap at the given top-left pixel position.
/// Honors the part's symmetry: HalfMirror parts are also rendered mirrored to
/// the right side; Symmetric parts are rendered once at the requested
/// position; AsymmetricFree parts are rendered once at the requested position
/// without mirroring.
pub fn render_part(bm: &mut Bitmap, part: &Part, top_left_x: u8, top_left_y: u8) {
    for (dy, &row) in part.rows.iter().enumerate() {
        for dx in 0..part.width_px {
            let bit_set = (row >> dx) & 1 == 1;
            if !bit_set { continue; }
            let x = top_left_x.saturating_add(dx);
            let y = top_left_y.saturating_add(dy as u8);
            if x < bm.w && y < bm.h { bm.set(x, y, true); }
            if matches!(part.symmetry, PartSymmetry::HalfMirror) {
                let mx = bm.w.saturating_sub(1).saturating_sub(x);
                if mx < bm.w && y < bm.h { bm.set(mx, y, true); }
            }
        }
    }
}

fn stage_index(s: Stage) -> u8 {
    match s {
        Stage::S0 => 0, Stage::S1 => 1, Stage::S2 => 2,
        Stage::S3 => 3, Stage::S4 => 4, Stage::S5 => 5, Stage::S6 => 6,
    }
}

/// Pick a part from a slice, filtering by `min_stage <= target_stage` and
/// rng-pick. Returns None if no parts satisfy the stage gate.
pub fn pick_part_for_stage<'a>(
    parts: &'a [Part],
    stage: Stage,
    rng: &mut SpikeRng,
) -> Option<&'a Part> {
    let target = stage_index(stage);
    let eligible: Vec<&Part> = parts.iter()
        .filter(|p| stage_index(p.min_stage) <= target)
        .collect();
    if eligible.is_empty() { None }
    else {
        let idx = (rng.next_u64() as usize) % eligible.len();
        Some(eligible[idx])
    }
}

/// Target body pixel height per stage. Bodies whose `height_px` is within ±2
/// pixels of this target are preferred for the stage.
fn stage_body_height_target(stage: Stage) -> u8 {
    let (_, h) = stage_grid_full(stage);
    h.saturating_sub(4) // head is 4 px; body roughly fills the rest with 1-px overlap
}

/// Pick a body part appropriate for the stage by both min_stage gate and
/// pixel-height tier. Falls back to any stage-eligible body if no body
/// matches the height tier.
fn pick_body_for_stage<'a>(
    parts: &'a [Part],
    stage: Stage,
    rng: &mut SpikeRng,
) -> Option<&'a Part> {
    let target = stage_body_height_target(stage);
    let target_idx = stage_index(stage);
    let tier_match: Vec<&Part> = parts.iter()
        .filter(|p| stage_index(p.min_stage) <= target_idx)
        .filter(|p| (p.height_px as i16 - target as i16).abs() <= 2)
        .collect();
    if !tier_match.is_empty() {
        let idx = (rng.next_u64() as usize) % tier_match.len();
        return Some(tier_match[idx]);
    }
    pick_part_for_stage(parts, stage, rng)
}

pub fn blueprint_for(
    species: SpeciesK,
    stage: Stage,
    seed: u64,
    catalog: &PartCatalog,
) -> PetBlueprint {
    let mut rng = SpikeRng::new(seed);
    let head = pick_part_for_stage(catalog.heads, stage, &mut rng)
        .expect("every species catalog must have at least one head part for every stage");
    let body = pick_body_for_stage(catalog.bodies, stage, &mut rng)
        .expect("every species catalog must have at least one body part for every stage");

    let stage_idx = stage_index(stage);
    let max_accessories = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 => 2,
        _         => 3,
    };
    let mut accessories: Vec<PartId> = Vec::with_capacity(max_accessories);
    let eligible_accessories: Vec<&Part> = catalog.accessories.iter()
        .filter(|p| stage_index(p.min_stage) <= stage_idx)
        .collect();
    if !eligible_accessories.is_empty() && max_accessories > 0 {
        let count = (rng.next_u64() as usize) % (max_accessories + 1);
        for _ in 0..count {
            let idx = (rng.next_u64() as usize) % eligible_accessories.len();
            accessories.push(eligible_accessories[idx].id);
        }
    }

    PetBlueprint {
        species,
        stage,
        selection: PartSelection { head: head.id, body: body.id, accessories },
    }
}

// ============================================================================

/// Constants that survived the parts pivot.
pub mod aesthetic {
    pub const EYE_ANCHOR_W_PX: u8 = 2;
    pub const EYE_ANCHOR_H_PX: u8 = 4;
}

/// Pixel dimensions of the full bitmap for each stage.
///
/// All widths are even (Braille is 2 wide per cell) and heights are multiples
/// of 4 (Braille is 4 tall per cell).
pub fn stage_grid_full(stage: Stage) -> (u8, u8) {
    match stage {
        Stage::S0 => (14, 12),
        Stage::S1 => (18, 16),
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => (22, 20),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn part_row_bit_decoding() {
        // A part with one 4-pixel-wide row "1011" (col 0, 2, 3 on; col 1 off)
        // should be encoded as 0b1101 (lsb is column 0).
        let row: u32 = 0b1101;
        assert!(row & 1 != 0, "col 0");
        assert!(row & 2 == 0, "col 1");
        assert!(row & 4 != 0, "col 2");
        assert!(row & 8 != 0, "col 3");
    }

    #[test]
    fn render_part_symmetric_writes_pixels() {
        // A 2×2 fully-filled symmetric part
        static ROWS: &[u32] = &[0b11, 0b11];
        let part = Part {
            id: PartId(1), rows: ROWS, width_px: 2, height_px: 2,
            anchor: Anchor::BodyCenter, min_stage: Stage::S0,
            symmetry: PartSymmetry::Symmetric, eye_anchors: None,
        };
        let mut bm = Bitmap::new(8, 8);
        render_part(&mut bm, &part, 3, 4);
        for y in 4..6 { for x in 3..5 { assert!(bm.get(x, y), "pixel ({x},{y}) should be on"); } }
    }

    #[test]
    fn render_part_half_mirror_writes_mirrored_pixels() {
        // A 2×1 half-mirror part: pixels at (0,0) and (1,0) on the left half
        static ROWS: &[u32] = &[0b11];
        let part = Part {
            id: PartId(2), rows: ROWS, width_px: 2, height_px: 1,
            anchor: Anchor::HeadTop, min_stage: Stage::S0,
            symmetry: PartSymmetry::HalfMirror, eye_anchors: None,
        };
        let mut bm = Bitmap::new(8, 4);
        render_part(&mut bm, &part, 0, 0);
        // Left half: cols 0-1 set on row 0
        assert!(bm.get(0, 0));
        assert!(bm.get(1, 0));
        // Right half: mirrored to cols 6-7 (w=8, so bm.w-1-x for x in {0,1} = {7,6})
        assert!(bm.get(7, 0));
        assert!(bm.get(6, 0));
    }

    #[test]
    fn pick_part_for_stage_returns_none_for_empty() {
        let mut rng = SpikeRng::new(7);
        let result = pick_part_for_stage(&[], Stage::S0, &mut rng);
        assert!(result.is_none());
    }

    #[test]
    fn pick_part_for_stage_filters_by_min_stage() {
        static ROWS: &[u32] = &[0b1];
        static PARTS: [Part; 2] = [
            Part { id: PartId(1), rows: ROWS, width_px: 1, height_px: 1,
                   anchor: Anchor::HeadCenter, min_stage: Stage::S0,
                   symmetry: PartSymmetry::Symmetric, eye_anchors: None },
            Part { id: PartId(2), rows: ROWS, width_px: 1, height_px: 1,
                   anchor: Anchor::HeadCenter, min_stage: Stage::S2,
                   symmetry: PartSymmetry::Symmetric, eye_anchors: None },
        ];
        let mut rng = SpikeRng::new(7);
        // At S0, only Part 1 is eligible.
        let p = pick_part_for_stage(&PARTS, Stage::S0, &mut rng).unwrap();
        assert_eq!(p.id, PartId(1));
        // At S2, both are eligible — the pick may be either.
        let p = pick_part_for_stage(&PARTS, Stage::S2, &mut rng).unwrap();
        assert!(p.id == PartId(1) || p.id == PartId(2));
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

/// A boolean bitmap, indexed [y][x] with x in 0..width_px, y in 0..height_px.
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap { pub w: u8, pub h: u8, pub cells: Vec<bool> }

impl Bitmap {
    pub fn new(w: u8, h: u8) -> Self { Self { w, h, cells: vec![false; (w as usize) * (h as usize)] } }
    pub fn idx(&self, x: u8, y: u8) -> usize { (y as usize) * (self.w as usize) + (x as usize) }
    pub fn get(&self, x: u8, y: u8) -> bool { self.cells[self.idx(x, y)] }
    pub fn set(&mut self, x: u8, y: u8, v: bool) { let i = self.idx(x, y); self.cells[i] = v; }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EyeAnchors { pub left: (u8, u8), pub right: (u8, u8) }

/// Place two symmetric eye anchors inside the head zone, separated from the
/// vertical midline by a margin. Returns top-left pixel of each anchor cell.
pub fn place_eye_anchors(full_w: u8, height_px: u8, head_zone_ratio: f32) -> EyeAnchors {
    let head_h = ((height_px as f32) * head_zone_ratio.max(aesthetic::EYE_ANCHOR_H_PX as f32 / height_px as f32)) as u8;
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

    const TEST_W: u8 = 14;
    const TEST_H: u8 = 12;
    const TEST_HEAD_RATIO: f32 = 0.30;

    #[test]
    fn eye_anchors_are_symmetric_around_midline() {
        let a = place_eye_anchors(TEST_W, TEST_H, TEST_HEAD_RATIO);
        let midline = (TEST_W / 2) as i32;
        let left_d = midline - (a.left.0 as i32 + aesthetic::EYE_ANCHOR_W_PX as i32);
        let right_d = a.right.0 as i32 - midline;
        assert_eq!(left_d, right_d);
    }

    #[test]
    fn reservation_clears_anchor_cells() {
        let mut bm = Bitmap::new(TEST_W, TEST_H);
        // Fill the bitmap so anchor reservation has pixels to clear.
        for y in 0..TEST_H {
            for x in 0..TEST_W {
                bm.set(x, y, true);
            }
        }
        let a = place_eye_anchors(TEST_W, TEST_H, TEST_HEAD_RATIO);
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
        eye: eyes[(rng.next_u64() as usize) % eyes.len()],
        mouth: mouths[(rng.next_u64() as usize) % mouths.len()],
        accent: accents[(rng.next_u64() as usize) % accents.len()],
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

    const TEST_W: u8 = 14;
    const TEST_H: u8 = 8;
    const TEST_HEAD_RATIO: f32 = 0.30;

    #[test]
    fn rendered_lines_have_expected_dimensions() {
        let mut bm = Bitmap::new(TEST_W, TEST_H);
        // Fill with some body pixels so the render has content.
        for y in 0..TEST_H {
            for x in 0..TEST_W {
                bm.set(x, y, true);
            }
        }
        let anchors = place_eye_anchors(TEST_W, TEST_H, TEST_HEAD_RATIO);
        reserve_eye_anchors(&mut bm, anchors);
        let features = FeatureGlyphPick { eye: "o", mouth: "w", accent: "·" };
        let lines = render_lines(&bm, anchors, &features);
        assert_eq!(lines.len(), (TEST_H / 4) as usize);
        for l in &lines { assert_eq!(l.chars().count(), (TEST_W / 2) as usize); }
    }
}

// =====================================================================
// Blob species catalog
// =====================================================================
//
// Blob aesthetic: round, smooth, soft. Heads are domes/bubbles; bodies are
// rounded blobs. Symmetric parts authored at full width (no half-mirror) to
// keep the pixel patterns readable.

mod blob {
    use super::*;

    // Eye anchors are part-relative pixel coords (top-left of each 2×4 cell).
    // Heads are 8 px wide × 4 px tall. Eye anchors at (1, 0) and (5, 0) place
    // each eye in its own braille char cell at the head's top row.

    pub static HEAD_DOME: Part = Part {
        id: PartId(100),
        rows: &[0b01111110, 0b11111111, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (1, 0), right: (5, 0) }),
    };

    pub static HEAD_BUBBLE: Part = Part {
        id: PartId(101),
        rows: &[0b00111100, 0b11111111, 0b11111111, 0b01111110],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (1, 0), right: (5, 0) }),
    };

    pub static HEAD_EGG: Part = Part {
        id: PartId(102),
        rows: &[0b00011000, 0b01111110, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (1, 0), right: (5, 0) }),
    };

    // Bodies are 10 px wide; composer centers them in the 14-px grid.

    pub static BODY_BUBBLE: Part = Part {
        id: PartId(200),
        rows: &[
            0b0111111110,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b0111111110,
            0b0011111100,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_COLUMN: Part = Part {
        id: PartId(201),
        rows: &[
            0b0011111100,
            0b0111111110,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b0111111110,
            0b0011111100,
            0b0011111100,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_BULB: Part = Part {
        id: PartId(202),
        rows: &[
            0b0011111100,
            0b0111111110,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b0111111110,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    // S1+ bodies: 12 px tall so head (4) + body (12) - overlap (1) ≈ 15 px,
    // close to the 16-px S1 grid height.
    pub static BODY_TALL_BUBBLE: Part = Part {
        id: PartId(203),
        rows: &[
            0b0111111110,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b0111111110,
            0b0011111100,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_TALL_GOURD: Part = Part {
        id: PartId(204),
        rows: &[
            0b0011111100,
            0b0011111100,
            0b0111111110,
            0b0111111110,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b0111111110,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    // S2+ body: 16 px tall to fill the 20-px S2 grid (4 head + 16 body - 1 overlap).
    pub static BODY_HUGE: Part = Part {
        id: PartId(205),
        rows: &[
            0b0111111110,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b1111111111,
            0b0111111110,
            0b0011111100,
        ],
        width_px: 10,
        height_px: 16,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static HEADS: &[Part] = &[HEAD_DOME, HEAD_BUBBLE, HEAD_EGG];
    pub static BODIES: &[Part] = &[
        BODY_BUBBLE, BODY_COLUMN, BODY_BULB,
        BODY_TALL_BUBBLE, BODY_TALL_GOURD, BODY_HUGE,
    ];
    pub static ACCESSORIES: &[Part] = &[];

    pub fn catalog() -> PartCatalog {
        PartCatalog { heads: HEADS, bodies: BODIES, accessories: ACCESSORIES }
    }
}

pub fn blob_catalog() -> PartCatalog { blob::catalog() }

pub fn catalog_for(species: SpeciesK) -> PartCatalog {
    match species {
        SpeciesK::Blob => blob_catalog(),
        _ => blob_catalog(), // placeholder until v2 Tasks 7a-7e fill in the others
    }
}

// =====================================================================
// Composition
// =====================================================================

pub fn compose_parts(blueprint: &PetBlueprint, catalog: &PartCatalog) -> Bitmap {
    let (w, h) = stage_grid_full(blueprint.stage);
    let mut bm = Bitmap::new(w, h);

    let body = catalog.bodies.iter().find(|p| p.id == blueprint.selection.body)
        .expect("body id must resolve in this catalog");
    let head = catalog.heads.iter().find(|p| p.id == blueprint.selection.head)
        .expect("head id must resolve in this catalog");

    // Head: centered horizontally, top-anchored at (0, 0).
    let head_full_w = match head.symmetry {
        PartSymmetry::HalfMirror => head.width_px * 2,
        _ => head.width_px,
    };
    let head_x = (w.saturating_sub(head_full_w)) / 2;
    let head_y = 0;
    render_part(&mut bm, head, head_x, head_y);

    // Body: centered horizontally, attached directly below the head (1-px
    // overlap to keep the silhouette continuous).
    let body_full_w = match body.symmetry {
        PartSymmetry::HalfMirror => body.width_px * 2,
        _ => body.width_px,
    };
    let body_x = (w.saturating_sub(body_full_w)) / 2;
    let body_y = head.height_px.saturating_sub(1);
    render_part(&mut bm, body, body_x, body_y);

    // Accessories placed in v2 Task 6.

    // Reserve eye anchors at the head's part-relative positions, offset.
    if let Some(anchors) = head.eye_anchors {
        let lx = head_x.saturating_add(anchors.left.0);
        let ly = head_y.saturating_add(anchors.left.1);
        let rx = head_x.saturating_add(anchors.right.0);
        let ry = head_y.saturating_add(anchors.right.1);
        reserve_eye_anchors(&mut bm, EyeAnchors { left: (lx, ly), right: (rx, ry) });
    }

    bm
}

pub fn generate_pet_lines(species: SpeciesK, stage: Stage, seed: u64) -> Vec<String> {
    let catalog = catalog_for(species);
    let blueprint = blueprint_for(species, stage, seed, &catalog);
    let bm = compose_parts(&blueprint, &catalog);

    // Resolve head anchors on the composed bitmap for the renderer.
    let (w, _h) = stage_grid_full(stage);
    let head = catalog.heads.iter().find(|p| p.id == blueprint.selection.head)
        .expect("head id must resolve");
    let head_full_w = match head.symmetry {
        PartSymmetry::HalfMirror => head.width_px * 2,
        _ => head.width_px,
    };
    let head_x = (w.saturating_sub(head_full_w)) / 2;
    let anchors = head.eye_anchors.map(|a| EyeAnchors {
        left: (head_x.saturating_add(a.left.0), a.left.1),
        right: (head_x.saturating_add(a.right.0), a.right.1),
    }).unwrap_or(EyeAnchors { left: (0, 0), right: (0, 0) });

    let mut rng = SpikeRng::new(seed);
    let features = pick_features(species, stage, &mut rng);
    render_lines(&bm, anchors, &features)
}

#[cfg(test)]
mod blob_tests {
    use super::*;

    #[test]
    fn blueprint_is_deterministic_per_seed() {
        let catalog = blob_catalog();
        let a = blueprint_for(SpeciesK::Blob, Stage::S1, 42, &catalog);
        let b = blueprint_for(SpeciesK::Blob, Stage::S1, 42, &catalog);
        assert_eq!(a.selection, b.selection);
    }

    #[test]
    fn generate_pet_lines_dimensions_match_stage() {
        let lines = generate_pet_lines(SpeciesK::Blob, Stage::S0, 42);
        let (w, h) = stage_grid_full(Stage::S0);
        assert_eq!(lines.len(), (h / 4) as usize);
        for l in &lines { assert_eq!(l.chars().count(), (w / 2) as usize); }
    }
}
