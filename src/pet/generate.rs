use crate::game::evolution::Stage;
use crate::pet::generation::{Species, StableRng};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartSelection {
    pub head: PartId,
    pub body: PartId,
    pub accessories: Vec<PartId>, // 0..=N
}

#[derive(Debug, Clone)]
pub struct PetBlueprint {
    pub species: Species,
    pub stage: Stage,
    pub selection: PartSelection,
}

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

fn stage_index(s: Stage) -> u8 {
    match s {
        Stage::S0 => 0, Stage::S1 => 1, Stage::S2 => 2,
        Stage::S3 => 3, Stage::S4 => 4, Stage::S5 => 5, Stage::S6 => 6,
    }
}

/// Pick a part from a slice, filtering by `min_stage <= target_stage` and
/// rng-pick. Returns None if no parts satisfy the stage gate.
pub(crate) fn pick_part_for_stage<'a>(
    parts: &'a [Part],
    stage: Stage,
    rng: &mut StableRng,
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
    rng: &mut StableRng,
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
    species: Species,
    stage: Stage,
    seed: u64,
    catalog: &PartCatalog,
) -> PetBlueprint {
    let mut rng = StableRng::new(seed);
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
// Bitmap
// ============================================================================

/// A boolean bitmap, indexed [y][x] with x in 0..width_px, y in 0..height_px.
#[derive(Debug, Clone, PartialEq)]
pub struct Bitmap { pub w: u8, pub h: u8, pub cells: Vec<bool> }

impl Bitmap {
    pub fn new(w: u8, h: u8) -> Self { Self { w, h, cells: vec![false; (w as usize) * (h as usize)] } }
    fn idx(&self, x: u8, y: u8) -> usize { (y as usize) * (self.w as usize) + (x as usize) }
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

// ============================================================================
// Braille encoding
// ============================================================================

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

// ============================================================================
// Feature glyphs
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureGlyphPick {
    pub eye: &'static str,    // single-glyph string (may be multi-byte)
    pub mouth: &'static str,
    pub accent: &'static str,
}

/// Stage-appropriate eye glyph alphabets per species.
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
        Species::Mech       => &["═", "─", "▪"],
        Species::Ghost      => &["", "·", "○"],
        Species::Glitch     => &["~", "≈", "─"],
        Species::Crystal    => &["◇", "◊"],
        Species::Blob       => &["w", "v", "ω"],
        Species::Fuzz       => &["w", "ᴗ", "ᵕ"],
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

pub(crate) fn pick_features(species: Species, stage: Stage, rng: &mut StableRng) -> FeatureGlyphPick {
    let eyes = eyes_for(species, stage);
    let mouths = mouths_for(species, stage);
    let accents = accents_for(species, stage);
    FeatureGlyphPick {
        eye: eyes[(rng.next_u64() as usize) % eyes.len()],
        mouth: mouths[(rng.next_u64() as usize) % mouths.len()],
        accent: accents[(rng.next_u64() as usize) % accents.len()],
    }
}

// ============================================================================
// CharGrid
// ============================================================================

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

// =====================================================================
// Mech species catalog
// =====================================================================
//
// Mech aesthetic: blocky, rectangular, hard 90-degree edges. Heads have flat
// tops; bodies are vertical rectangles. No rounded corners anywhere.

mod mech {
    use super::*;

    pub static HEAD_RECT: Part = Part {
        id: PartId(300),
        rows: &[0b11111111, 0b11111111, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_ANTENNA: Part = Part {
        id: PartId(301),
        rows: &[0b00011000, 0b11111111, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_DOME_FLAT: Part = Part {
        id: PartId(302),
        rows: &[0b01111110, 0b11111111, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    // Mech bodies: 10 px wide rectangles, flat tops/bottoms.
    pub static BODY_BLOCK_S0: Part = Part {
        id: PartId(400),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_SEGMENTED_S0: Part = Part {
        id: PartId(401),
        rows: &[
            0b1111111111, 0b1111111111, 0b1000000001, 0b1111111111,
            0b1111111111, 0b1000000001, 0b1111111111, 0b1111111111,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_BLOCK_TALL: Part = Part {
        id: PartId(402),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_SEGMENTED_TALL: Part = Part {
        id: PartId(403),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1000000001,
            0b1111111111, 0b1111111111, 0b1000000001, 0b1111111111,
            0b1111111111, 0b1000000001, 0b1111111111, 0b1111111111,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_BLOCK_HUGE: Part = Part {
        id: PartId(404),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
        ],
        width_px: 10,
        height_px: 16,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static HEADS: &[Part] = &[HEAD_RECT, HEAD_ANTENNA, HEAD_DOME_FLAT];
    pub static BODIES: &[Part] = &[
        BODY_BLOCK_S0, BODY_SEGMENTED_S0,
        BODY_BLOCK_TALL, BODY_SEGMENTED_TALL,
        BODY_BLOCK_HUGE,
    ];

    pub fn catalog() -> PartCatalog {
        PartCatalog { heads: HEADS, bodies: BODIES, accessories: &[] }
    }
}

pub fn mech_catalog() -> PartCatalog { mech::catalog() }

// =====================================================================
// Ghost species catalog
// =====================================================================
//
// Ghost aesthetic: tall, wispy, narrower head, body tapers to nothing at the
// bottom (no defined feet). Hollow/wave silhouettes.

mod ghost {
    use super::*;

    pub static HEAD_VEIL: Part = Part {
        id: PartId(500),
        rows: &[0b00011000, 0b00111100, 0b01111110, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_HOLLOW: Part = Part {
        id: PartId(501),
        rows: &[0b00111100, 0b01100110, 0b11000011, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_SHARP: Part = Part {
        id: PartId(502),
        rows: &[0b00011000, 0b00111100, 0b01111110, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    // Ghost bodies: tapering, wavy bottoms (no defined feet).
    pub static BODY_WAVE_S0: Part = Part {
        id: PartId(600),
        rows: &[
            0b1111111111, 0b1111111111, 0b0111111110, 0b0011111100,
            0b0011111100, 0b0010110100, 0b0001100100, 0b0001010100,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_SMOKE_S0: Part = Part {
        id: PartId(601),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b0111111110,
            0b0011111100, 0b0001111000, 0b0011001100, 0b0010000100,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_WAVE_TALL: Part = Part {
        id: PartId(602),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b0111111110, 0b0011111100, 0b0011111100,
            0b0010111100, 0b0010110100, 0b0001100100, 0b0001000100,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_SMOKE_TALL: Part = Part {
        id: PartId(603),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b0111111110, 0b0011111100,
            0b0001111000, 0b0011001100, 0b0010000100, 0b0001000100,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_WAVE_HUGE: Part = Part {
        id: PartId(604),
        rows: &[
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b0111111110,
            0b0111111110, 0b0011111100, 0b0011111100, 0b0010111100,
            0b0010110100, 0b0001100100, 0b0001010100, 0b0001000100,
        ],
        width_px: 10,
        height_px: 16,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static HEADS: &[Part] = &[HEAD_VEIL, HEAD_HOLLOW, HEAD_SHARP];
    pub static BODIES: &[Part] = &[
        BODY_WAVE_S0, BODY_SMOKE_S0,
        BODY_WAVE_TALL, BODY_SMOKE_TALL,
        BODY_WAVE_HUGE,
    ];

    pub fn catalog() -> PartCatalog {
        PartCatalog { heads: HEADS, bodies: BODIES, accessories: &[] }
    }
}

pub fn ghost_catalog() -> PartCatalog { ghost::catalog() }

// =====================================================================
// Crystal species catalog
// =====================================================================
//
// Crystal aesthetic: faceted, geometric. Pointed/diamond heads, prism bodies
// with angular sides.

mod crystal {
    use super::*;

    pub static HEAD_DIAMOND: Part = Part {
        id: PartId(700),
        rows: &[0b00011000, 0b00111100, 0b01111110, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_HEX: Part = Part {
        id: PartId(701),
        rows: &[0b00111100, 0b01111110, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_SHARD: Part = Part {
        id: PartId(702),
        rows: &[0b00011000, 0b00100100, 0b01000010, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    // Crystal bodies: diamond/prism silhouettes — narrow waist or sharp angles.
    pub static BODY_PRISM_S0: Part = Part {
        id: PartId(800),
        rows: &[
            0b0011111100, 0b0111111110, 0b1111111111, 0b1111111111,
            0b1111111111, 0b0111111110, 0b0011111100, 0b0001111000,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_FACET_S0: Part = Part {
        id: PartId(801),
        rows: &[
            0b0111111110, 0b1111111111, 0b1111111111, 0b0111111110,
            0b0011111100, 0b0111111110, 0b1111111111, 0b1111111111,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_PRISM_TALL: Part = Part {
        id: PartId(802),
        rows: &[
            0b0011111100, 0b0111111110, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b0111111110, 0b0011111100,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_CLUSTER_TALL: Part = Part {
        id: PartId(803),
        rows: &[
            0b0111111110, 0b1111111111, 0b1111111111, 0b0111111110,
            0b0011111100, 0b0011111100, 0b0111111110, 0b1111111111,
            0b1111111111, 0b1111111111, 0b0111111110, 0b0011111100,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_PRISM_HUGE: Part = Part {
        id: PartId(804),
        rows: &[
            0b0011111100, 0b0111111110, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b0111111110, 0b0011111100,
        ],
        width_px: 10,
        height_px: 16,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static HEADS: &[Part] = &[HEAD_DIAMOND, HEAD_HEX, HEAD_SHARD];
    pub static BODIES: &[Part] = &[
        BODY_PRISM_S0, BODY_FACET_S0,
        BODY_PRISM_TALL, BODY_CLUSTER_TALL,
        BODY_PRISM_HUGE,
    ];

    pub fn catalog() -> PartCatalog {
        PartCatalog { heads: HEADS, bodies: BODIES, accessories: &[] }
    }
}

pub fn crystal_catalog() -> PartCatalog { crystal::catalog() }

// =====================================================================
// Glitch species catalog
// =====================================================================
//
// Glitch aesthetic: fragmented, irregular, scan-line patterns, scattered
// missing pixels. Reads as "broken creature."

mod glitch {
    use super::*;

    pub static HEAD_FRAG: Part = Part {
        id: PartId(900),
        rows: &[0b01111110, 0b11011011, 0b11111111, 0b10111101],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_SCRAMBLE: Part = Part {
        id: PartId(901),
        rows: &[0b00111100, 0b11111111, 0b10100101, 0b11011011],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_NOISE: Part = Part {
        id: PartId(902),
        rows: &[0b01101110, 0b11101111, 0b10111101, 0b11011011],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    // Glitch bodies: scan-line patterns, scattered gaps.
    pub static BODY_SCAN_S0: Part = Part {
        id: PartId(1000),
        rows: &[
            0b1111111111, 0b0000000000, 0b1111111111, 0b1111111111,
            0b0011111100, 0b1111111111, 0b0000000000, 0b1111111111,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_FRAG_S0: Part = Part {
        id: PartId(1001),
        rows: &[
            0b1111111111, 0b1011011101, 0b1111111111, 0b1101110111,
            0b1111111111, 0b1011101101, 0b1111111111, 0b1110111011,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_SCAN_TALL: Part = Part {
        id: PartId(1002),
        rows: &[
            0b1111111111, 0b0000000000, 0b1111111111, 0b1111111111,
            0b0011111100, 0b1111111111, 0b0000000000, 0b1111111111,
            0b1111111111, 0b0011111100, 0b1111111111, 0b0000000000,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_FRAG_TALL: Part = Part {
        id: PartId(1003),
        rows: &[
            0b1111111111, 0b1011011101, 0b1111111111, 0b1101110111,
            0b1111111111, 0b1011101101, 0b1111111111, 0b1110111011,
            0b1111111111, 0b1011011101, 0b1111111111, 0b1101110111,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_SCAN_HUGE: Part = Part {
        id: PartId(1004),
        rows: &[
            0b1111111111, 0b0000000000, 0b1111111111, 0b1111111111,
            0b0011111100, 0b1111111111, 0b0000000000, 0b1111111111,
            0b1111111111, 0b0011111100, 0b1111111111, 0b0000000000,
            0b1111111111, 0b1111111111, 0b0011111100, 0b1111111111,
        ],
        width_px: 10,
        height_px: 16,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static HEADS: &[Part] = &[HEAD_FRAG, HEAD_SCRAMBLE, HEAD_NOISE];
    pub static BODIES: &[Part] = &[
        BODY_SCAN_S0, BODY_FRAG_S0,
        BODY_SCAN_TALL, BODY_FRAG_TALL,
        BODY_SCAN_HUGE,
    ];

    pub fn catalog() -> PartCatalog {
        PartCatalog { heads: HEADS, bodies: BODIES, accessories: &[] }
    }
}

pub fn glitch_catalog() -> PartCatalog { glitch::catalog() }

// =====================================================================
// Fuzz species catalog
// =====================================================================
//
// Fuzz aesthetic: fluffy, irregular, slightly bigger heads with edge fuzz.

mod fuzz {
    use super::*;

    pub static HEAD_FLUFF: Part = Part {
        id: PartId(1100),
        rows: &[0b00111100, 0b01111110, 0b11111111, 0b01111110],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_TUFT: Part = Part {
        id: PartId(1101),
        rows: &[0b00100100, 0b01111110, 0b11111111, 0b11111111],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    pub static HEAD_PUFF: Part = Part {
        id: PartId(1102),
        rows: &[0b01011010, 0b01111110, 0b11111111, 0b01111110],
        width_px: 8,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: Some(EyeAnchors { left: (0, 0), right: (6, 0) }),
    };

    // Fuzz bodies: wider, rounded, with edge irregularity in cols 2-5.
    pub static BODY_FLUFFY_S0: Part = Part {
        id: PartId(1200),
        rows: &[
            0b1101111011, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1101111011, 0b0111111110,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_OVAL_S0: Part = Part {
        id: PartId(1201),
        rows: &[
            0b0111111110, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b0111111110,
        ],
        width_px: 10,
        height_px: 8,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_FLUFFY_TALL: Part = Part {
        id: PartId(1202),
        rows: &[
            0b1101111011, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1101111011, 0b0111111110,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_OVAL_TALL: Part = Part {
        id: PartId(1203),
        rows: &[
            0b0111111110, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b0111111110,
        ],
        width_px: 10,
        height_px: 12,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static BODY_FLUFFY_HUGE: Part = Part {
        id: PartId(1204),
        rows: &[
            0b1101111011, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1111111111, 0b1111111111,
            0b1111111111, 0b1111111111, 0b1101111011, 0b0111111110,
        ],
        width_px: 10,
        height_px: 16,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static HEADS: &[Part] = &[HEAD_FLUFF, HEAD_TUFT, HEAD_PUFF];
    pub static BODIES: &[Part] = &[
        BODY_FLUFFY_S0, BODY_OVAL_S0,
        BODY_FLUFFY_TALL, BODY_OVAL_TALL,
        BODY_FLUFFY_HUGE,
    ];

    pub fn catalog() -> PartCatalog {
        PartCatalog { heads: HEADS, bodies: BODIES, accessories: &[] }
    }
}

pub fn fuzz_catalog() -> PartCatalog { fuzz::catalog() }

pub fn catalog_for(species: Species) -> PartCatalog {
    match species {
        Species::Blob    => blob_catalog(),
        Species::Mech    => mech_catalog(),
        Species::Ghost   => ghost_catalog(),
        Species::Crystal => crystal_catalog(),
        Species::Glitch  => glitch_catalog(),
        Species::Fuzz    => fuzz_catalog(),
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

    // Head: centered horizontally, top-anchored. We snap head_x down to an
    // even value so eye-anchor reservations align with braille character
    // boundaries (each braille char covers 2×4 px; an odd anchor x would
    // straddle two chars and leave artifacts at the head's left/right edges).
    let head_full_w = match head.symmetry {
        PartSymmetry::HalfMirror => head.width_px * 2,
        _ => head.width_px,
    };
    let head_x = ((w.saturating_sub(head_full_w)) / 2) & !1;
    let head_y = 0;
    render_part(&mut bm, head, head_x, head_y);

    // Body: centered horizontally (also snapped to even x), attached directly
    // below the head with a 1-px overlap to keep the silhouette continuous.
    let body_full_w = match body.symmetry {
        PartSymmetry::HalfMirror => body.width_px * 2,
        _ => body.width_px,
    };
    let body_x = ((w.saturating_sub(body_full_w)) / 2) & !1;
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

pub fn generate_pet_lines(species: Species, stage: Stage, seed: u64) -> Vec<String> {
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
    let head_x = ((w.saturating_sub(head_full_w)) / 2) & !1;
    let anchors = head.eye_anchors.map(|a| EyeAnchors {
        left: (head_x.saturating_add(a.left.0), a.left.1),
        right: (head_x.saturating_add(a.right.0), a.right.1),
    }).unwrap_or(EyeAnchors { left: (0, 0), right: (0, 0) });

    let mut rng = StableRng::new(seed);
    let features = pick_features(species, stage, &mut rng);
    render_lines(&bm, anchors, &features)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Determinism + dimension tests for the pipeline.

    #[test]
    fn pipeline_deterministic_per_seed() {
        let a = generate_pet_lines(Species::Blob, Stage::S0, 42);
        let b = generate_pet_lines(Species::Blob, Stage::S0, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn species_produce_distinct_outputs_at_s2() {
        let blob = generate_pet_lines(Species::Blob, Stage::S2, 42);
        let mech = generate_pet_lines(Species::Mech, Stage::S2, 42);
        let ghost = generate_pet_lines(Species::Ghost, Stage::S2, 42);
        assert_ne!(blob, mech);
        assert_ne!(blob, ghost);
        assert_ne!(mech, ghost);
    }

    #[test]
    fn pipeline_dimensions_match_stage() {
        let lines = generate_pet_lines(Species::Mech, Stage::S2, 99);
        let (w, h) = stage_grid_full(Stage::S2);
        assert_eq!(lines.len(), (h / 4) as usize);
        for l in &lines {
            assert_eq!(l.chars().count(), (w / 2) as usize);
        }
    }

    #[test]
    fn full_widths_are_even_and_heights_multiple_of_4() {
        for s in [Stage::S0, Stage::S1, Stage::S2, Stage::S3, Stage::S4, Stage::S5, Stage::S6] {
            let (w, h) = stage_grid_full(s);
            assert_eq!(w % 2, 0, "width not even for {:?}: {w}", s);
            assert_eq!(h % 4, 0, "height not /4 for {:?}: {h}", s);
        }
    }

    #[test]
    fn rng_is_deterministic_per_seed() {
        let mut a = StableRng::new(42);
        let mut b = StableRng::new(42);
        for _ in 0..1000 { assert_eq!(a.next_u64(), b.next_u64()); }
    }

    #[test]
    fn render_part_symmetric_writes_pixels() {
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
        static ROWS: &[u32] = &[0b11];
        let part = Part {
            id: PartId(2), rows: ROWS, width_px: 2, height_px: 1,
            anchor: Anchor::HeadTop, min_stage: Stage::S0,
            symmetry: PartSymmetry::HalfMirror, eye_anchors: None,
        };
        let mut bm = Bitmap::new(8, 4);
        render_part(&mut bm, &part, 0, 0);
        assert!(bm.get(0, 0));
        assert!(bm.get(1, 0));
        assert!(bm.get(7, 0));
        assert!(bm.get(6, 0));
    }

    #[test]
    fn empty_block_is_blank_braille() {
        let bm = Bitmap::new(2, 4);
        assert_eq!(braille_block(&bm, 0, 0), '\u{2800}');
    }

    #[test]
    fn full_block_is_solid_braille() {
        let mut bm = Bitmap::new(2, 4);
        for y in 0..4 { for x in 0..2 { bm.set(x, y, true); } }
        assert_eq!(braille_block(&bm, 0, 0), '\u{28FF}');
    }
}
