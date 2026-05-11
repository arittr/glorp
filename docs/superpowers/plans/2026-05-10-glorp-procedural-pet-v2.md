# Glorp Compositional-Parts Pet (v2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development for the mechanical tasks; the controller is expected to do part-authoring tasks inline because they're aesthetic-iteration-heavy and subagent overhead doesn't pay off there. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `src/pet/art.rs` with species-specific compositional-parts generation. Every species has its own non-overlapping authored library of small glyph parts; each pet's seed picks one head + one body + 0-N accessories from its species' catalog, composed into a Braille bitmap.

**Architecture:** A new `src/pet/generate.rs` exposes `PartCatalog` per species, a `compose_parts(species, seed, stage) -> Bitmap` composer, and a top-level `generate_pet_lines(species, stage, seed) -> Vec<String>` that wraps composition + Braille encoding + feature glyph overlay. `src/pet/render.rs` keeps its public surface (`render_pet`, `RenderedPet`, `StyledSegment`) and delegates internals to `generate.rs`.

**Tech Stack:** Rust 2021, ratatui 0.29 (unchanged), in-tree `StableRng`, insta for snapshot tests.

**Source spec:** `docs/superpowers/specs/2026-05-10-glorp-frontend-overhaul-design.md` (updated 2026-05-10 to reflect the parts pivot).

**Supersedes:** `docs/superpowers/plans/2026-05-10-glorp-procedural-pet.md` — the algorithmic spike implemented Tasks 1–22 of that plan before failing the species-distinctness gate. The compositional-parts approach uses a few primitives from that work (Bitmap, braille encoder, eye anchor concept, feature glyph subsets, CharGrid, render_lines, SpikeRng) but discards the algorithmic silhouette generator.

**This plan covers Phase 0 (parts spike) + Phase 1 (production promotion).** Phases 2-6 (layout overhaul, animator, tachyonfx, mouse-tracked eyes, polish) get separate plans after this one lands.

---

## File map

**Modified:**
- `examples/pet_gallery.rs` — rewritten as a parts-based gallery printer. The algorithmic generator code is removed in Task 1; new types and catalogs grow across subsequent tasks.
- `src/pet/render.rs` — keep public types (`RenderedPet`, `StyledSegment`, `PaletteRoleName`, `PaletteRoles`, `PaletteRole`, `AnimationFrame`, `AnimationProfile`) and public function `render_pet`. Replace internals to consume `pet::generate::generate_pet_lines`.
- `src/pet/generation.rs` — promote `StableRng` to `pub(crate)`, add `next_f32_unit`, `next_bias`, `next_signed_unit`, `fnv1a64` made `pub(crate)`. Otherwise unchanged.
- `src/pet/mod.rs` — drop `pub mod art;`, add `pub mod generate;`.

**New:**
- `src/pet/generate.rs` — promoted version of the working spike. Hosts `PartCatalog`, the six species catalogs, the composer, blueprint derivation, and the top-level pipeline.

**Deleted:**
- `src/pet/art.rs` — replaced by the parts catalog. All callers migrated to `pet::generate`.

---

## Phase 0 — Parts spike (gate)

### Task 1: Reset the spike file and scaffold parts primitives

**Files:**
- Rewrite: `examples/pet_gallery.rs` (large rewrite — the algorithmic generator code is deleted and replaced with parts scaffolding).

This is one larger task because most of the carry-forward primitives are already in the file from the previous plan's tasks. The deletion is mechanical; the new additions are small.

- [ ] **Step 1: Delete the algorithmic generator code**

Remove from `examples/pet_gallery.rs`:
- The `aesthetic` module (most of it — see Step 2 for what to keep).
- `gaussian_envelope`, `head_zone_gain`, `corner_taper`, `coherent_noise`, `fill_probability`.
- `sample_silhouette`, the `Bitmap` impl's `filled_ratio` method (parts don't need fill-ratio rejection).
- `OrnamentKind`, `ornament_pattern`, `SYM_ORNAMENT_KINDS`, `add_symmetric_ornaments`, `add_asymmetric_ornaments`, `find_top_filled`, `place_ornament` — replaced by part composition.
- `species_baseline`, `MutationVector` struct, `derive_mutation_vector`, `apply_mutation` — replaced by `PartSelection`/`EvolutionPath`.
- `blueprint_for` (current version), `generate_pet_lines` (current version) — to be rewritten in later tasks.
- All test modules whose tests reference deleted functions (`envelope_tests`, `head_zone_tests`, `taper_tests`, `noise_tests`, `fill_tests`, `silhouette_tests`, `ornament_tests`, `async_tests`, `baseline_tests`, `mutation_tests`, `pipeline_tests`).

Keep: `SpikeRng`, `Bitmap` (struct + `new` + `idx` + `get` + `set`), `Stage`, `SpeciesK`, `EyeAnchors` + `place_eye_anchors` + `reserve_eye_anchors`, `braille_block`, `encode_braille`, `FeatureGlyphPick` + `eyes_for`/`mouths_for`/`accents_for` + `pick_features`, `CharGrid` + `render_lines`, `stage_grid_full`. Also keep the test modules whose tests reference only kept code (e.g., `rng_*`, `braille_tests`, `anchor_tests`, `feature_tests`, `render_tests`, `stage_tests`).

- [ ] **Step 2: Update `aesthetic` constants to the parts-relevant subset**

Replace the `aesthetic` module body with:

```rust
/// Constants that survived the parts pivot.
pub mod aesthetic {
    pub const EYE_ANCHOR_W_PX: u8 = 2;
    pub const EYE_ANCHOR_H_PX: u8 = 4;
}
```

(Remove `aesthetic_tests` — it now only tests trivial constants. Or update it to a single trivial assertion. Pick the cleanest option that doesn't trigger a clippy warning.)

- [ ] **Step 3: Update `stage_grid_full` to the new dimensions**

Per the updated spec:

```rust
pub fn stage_grid_full(stage: Stage) -> (u8, u8) {
    match stage {
        Stage::S0 => (14, 12),
        Stage::S1 => (18, 16),
        Stage::S2 | Stage::S3 | Stage::S4 | Stage::S5 | Stage::S6 => (22, 20),
    }
}
```

S2 grows from 16→20 px tall because parts naturally use more vertical space than the old Gaussian silhouette did. Width is unchanged. Update the `full_widths_are_even_and_heights_multiple_of_4` test (it should still pass — 20 is divisible by 4).

- [ ] **Step 4: Replace `main` with a placeholder**

```rust
fn main() {
    println!("parts_gallery — Phase 0 compositional parts spike");
    println!("(catalogs not yet defined — gallery printer wired in Task 8)");
}
```

The full gallery printer is reintroduced in Task 8 once the composer and at least one catalog exist.

- [ ] **Step 5: Build + test**

Run: `cargo build --example pet_gallery`
Run: `cargo test --example pet_gallery`
Expected: clean build; the remaining tests pass (the deleted test modules are gone, so test count drops to whatever the carry-forward primitives have — roughly 12-15 tests).

- [ ] **Step 6: Commit**

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): reset spike to parts scaffolding; delete algorithmic generator"
```

### Task 2: Add `Part` and `PartCatalog` types

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Define the types**

Append (after `SpikeRng` impl, before existing tests):

```rust
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
/// Pixels are stored row-major in `rows`: each u32 represents one row, with bit
/// 0 (lsb) = column 0 of that row. `width_px` columns, `rows.len()` rows.
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
```

- [ ] **Step 2: Add a smoke test**

Inside the existing `tests` module:

```rust
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
```

- [ ] **Step 3: Build + test + commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): Part, PartCatalog, Anchor, PartSymmetry types"
```

### Task 3: Part rendering primitive — `render_part`

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Add `render_part` and tests**

Append (after `Part`/`PartCatalog` types, before tests):

```rust
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
```

Add to the existing `tests` module:

```rust
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
```

- [ ] **Step 2: Build + test + commit**

```bash
cargo test --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): render_part primitive with symmetry handling"
```

### Task 4: Blueprint, PartSelection, and EvolutionPath

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Replace the old PetBlueprint definition (and add new types)**

The old plan defined `PetBlueprint { species, stage, silhouette, mutation_vector }`. That should already be deleted by Task 1. Add the new types:

```rust
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

/// Pick a part by index from a slice, filtering by min_stage and rng-pick.
/// Returns None if no parts in the slice satisfy `min_stage <= stage`.
pub fn pick_part_for_stage<'a>(
    parts: &'a [Part],
    stage: Stage,
    rng: &mut SpikeRng,
) -> Option<&'a Part> {
    let eligible: Vec<&Part> = parts.iter()
        .filter(|p| stage_index(p.min_stage) <= stage_index(stage))
        .collect();
    if eligible.is_empty() { None }
    else { Some(eligible[rng.next_u64() as usize % eligible.len()]) }
}

fn stage_index(s: Stage) -> u8 {
    match s {
        Stage::S0 => 0, Stage::S1 => 1, Stage::S2 => 2,
        Stage::S3 => 3, Stage::S4 => 4, Stage::S5 => 5, Stage::S6 => 6,
    }
}
```

`SpikeRng` needs `next_u64()` (already exists). If the previous spike had a `next_usize_capped` helper that survived the Task 1 cleanup, use it instead — it's slightly cleaner. Adapt the body accordingly.

- [ ] **Step 2: Add `blueprint_for(species, stage, seed, catalog) -> PetBlueprint`**

```rust
pub fn blueprint_for(
    species: SpeciesK,
    stage: Stage,
    seed: u64,
    catalog: &PartCatalog,
) -> PetBlueprint {
    let mut rng = SpikeRng::new(seed);
    let head = pick_part_for_stage(catalog.heads, stage, &mut rng)
        .expect("every species catalog must have at least one head part for every stage");
    let body = pick_part_for_stage(catalog.bodies, stage, &mut rng)
        .expect("every species catalog must have at least one body part for every stage");

    let stage_idx = stage_index(stage);
    let max_accessories = match stage {
        Stage::S0 => 0,
        Stage::S1 => 1,
        Stage::S2 => 2,
        _         => 3,
    };
    let mut accessories = Vec::with_capacity(max_accessories);
    let eligible_accessories: Vec<&Part> = catalog.accessories.iter()
        .filter(|p| stage_index(p.min_stage) <= stage_idx)
        .collect();
    if !eligible_accessories.is_empty() {
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
```

- [ ] **Step 3: Test**

```rust
#[test]
fn blueprint_is_deterministic_per_seed() {
    let catalog = blob_catalog();
    let a = blueprint_for(SpeciesK::Blob, Stage::S1, 42, &catalog);
    let b = blueprint_for(SpeciesK::Blob, Stage::S1, 42, &catalog);
    assert_eq!(a.selection, b.selection);
}
```

This test won't compile until Task 5 adds `blob_catalog`. Mark the test `#[ignore]` for now or stub the test to assert a simpler property; remove the `#[ignore]` in Task 5.

- [ ] **Step 4: Build + commit**

```bash
cargo build --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): PartSelection, PetBlueprint, blueprint_for"
```

### Task 5: Authored Blob part catalog (proof-of-concept)

**This is the first species catalog — authoring is iterative and visual. Do this task inline (controller), not via a subagent.** Iterate by adding a part → running gallery → adjusting until Blobs look distinct and rounded.

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Author 3 head parts for Blob**

Add (after the `Part` types, before tests):

```rust
// === Blob species catalog ===
//
// Blob aesthetic: round, smooth, soft. Heads are domes/bubbles; bodies are
// rounded blobs; accessories are droplets and gentle bumps.

mod blob {
    use super::*;

    // Head 1: round dome, 6×4 px (half-mirror — left side authored).
    // Reading rows (each is the left half, bit 0 = column 0):
    //   . X X .    →  0b0110
    //   X X X X    →  0b1111
    //   X X X X    →  0b1111
    //   X X X X    →  0b1111
    pub static HEAD_DOME: Part = Part {
        id: PartId(100),
        rows: &[0b0110, 0b1111, 0b1111, 0b1111],
        width_px: 4,  // half-width; composer mirrors to full 8
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::HalfMirror,
        eye_anchors: Some(EyeAnchors { left: (1, 1), right: (6, 1) }), // post-mirror absolute coords
    };

    // Head 2: bubble with rounded top, 6×4 px.
    //   X X X .
    //   X X X X
    //   X X X X
    //   . X X .
    pub static HEAD_BUBBLE: Part = Part {
        id: PartId(101),
        rows: &[0b0111, 0b1111, 0b1111, 0b0110],
        width_px: 4,
        height_px: 4,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::HalfMirror,
        eye_anchors: Some(EyeAnchors { left: (1, 1), right: (6, 1) }),
    };

    // Head 3: tall dome with neck, 6×5 px (S1+).
    //   . X X .
    //   X X X X
    //   X X X X
    //   . X X .
    //   . X X .
    pub static HEAD_NECKED: Part = Part {
        id: PartId(102),
        rows: &[0b0110, 0b1111, 0b1111, 0b0110, 0b0110],
        width_px: 4,
        height_px: 5,
        anchor: Anchor::HeadCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::HalfMirror,
        eye_anchors: Some(EyeAnchors { left: (1, 1), right: (6, 1) }),
    };

    pub static HEADS: &[Part] = &[HEAD_DOME, HEAD_BUBBLE, HEAD_NECKED];
}
```

(The 8 vs 4 width and the eye_anchor coords need adjustment per spike iteration. The encoding above puts eyes at the head-cell row 1, columns 1 and 6 within a mirrored 8-wide head. Verify visually after Step 4.)

- [ ] **Step 2: Author 3 body parts for Blob**

```rust
mod blob {
    // ... (heads from Step 1)

    // Body 1: round bubble, 6×4 px half-mirror.
    pub static BODY_BUBBLE: Part = Part {
        id: PartId(200),
        rows: &[0b1111, 0b1111, 0b1111, 0b0111],
        width_px: 4,
        height_px: 4,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::HalfMirror,
        eye_anchors: None,
    };

    // Body 2: tall column, 4×6 px (S1+).
    pub static BODY_COLUMN: Part = Part {
        id: PartId(201),
        rows: &[0b0111, 0b1111, 0b1111, 0b1111, 0b1111, 0b0111],
        width_px: 4,
        height_px: 6,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::HalfMirror,
        eye_anchors: None,
    };

    // Body 3: wider bulb, 6×4 px (S0+).
    pub static BODY_BULB: Part = Part {
        id: PartId(202),
        rows: &[0b0111, 0b1111, 0b1111, 0b0011],
        width_px: 4,
        height_px: 4,
        anchor: Anchor::BodyCenter,
        min_stage: Stage::S0,
        symmetry: PartSymmetry::HalfMirror,
        eye_anchors: None,
    };

    pub static BODIES: &[Part] = &[BODY_BUBBLE, BODY_COLUMN, BODY_BULB];

    pub fn catalog() -> PartCatalog {
        PartCatalog {
            heads: HEADS,
            bodies: BODIES,
            accessories: &[],  // accessories added in Task 6
        }
    }
}

pub fn blob_catalog() -> PartCatalog { blob::catalog() }
```

- [ ] **Step 3: Add `compose_parts(blueprint, catalog) -> Bitmap`**

```rust
pub fn compose_parts(blueprint: &PetBlueprint, catalog: &PartCatalog) -> Bitmap {
    let (w, h) = stage_grid_full(blueprint.stage);
    let mut bm = Bitmap::new(w, h);

    let body = catalog.bodies.iter().find(|p| p.id == blueprint.selection.body)
        .expect("body id must resolve in this catalog");
    let head = catalog.heads.iter().find(|p| p.id == blueprint.selection.head)
        .expect("head id must resolve in this catalog");

    // Body: centered horizontally, bottom-anchored to (h - body_height).
    let body_full_w = body.width_px * 2; // half-mirror double-width
    let body_x = (w.saturating_sub(body_full_w)) / 2;
    let body_y = h.saturating_sub(body.height_px);
    render_part(&mut bm, body, body_x, body_y);

    // Head: centered horizontally, top-anchored.
    let head_full_w = head.width_px * 2;
    let head_x = (w.saturating_sub(head_full_w)) / 2;
    let head_y = 0;
    render_part(&mut bm, head, head_x, head_y);

    // Accessories (skipped at Task 5 — no accessories yet).
    for _acc_id in &blueprint.selection.accessories {
        // Implemented in Task 6.
    }

    // Reserve eye anchors from the head part.
    if let Some(anchors) = head.eye_anchors {
        let lx = head_x.saturating_add(anchors.left.0);
        let ly = head_y.saturating_add(anchors.left.1);
        let rx = head_x.saturating_add(anchors.right.0);
        let ry = head_y.saturating_add(anchors.right.1);
        reserve_eye_anchors(&mut bm, EyeAnchors { left: (lx, ly), right: (rx, ry) });
    }

    bm
}
```

- [ ] **Step 4: Rewrite `generate_pet_lines` and the gallery printer**

```rust
pub fn generate_pet_lines(species: SpeciesK, stage: Stage, seed: u64) -> Vec<String> {
    let catalog = catalog_for(species);
    let blueprint = blueprint_for(species, stage, seed, &catalog);
    let bm = compose_parts(&blueprint, &catalog);

    // Resolve head's eye anchor positions on the composed bitmap (same math
    // as compose_parts; refactor into a helper if it grows).
    let (w, _h) = stage_grid_full(stage);
    let head = catalog.heads.iter().find(|p| p.id == blueprint.selection.head).unwrap();
    let head_full_w = head.width_px * 2;
    let head_x = (w.saturating_sub(head_full_w)) / 2;
    let head_y = 0u8;
    let anchors = head.eye_anchors.map(|a| EyeAnchors {
        left: (head_x.saturating_add(a.left.0), head_y.saturating_add(a.left.1)),
        right: (head_x.saturating_add(a.right.0), head_y.saturating_add(a.right.1)),
    }).unwrap_or(EyeAnchors { left: (0, 0), right: (0, 0) });

    let mut rng = SpikeRng::new(seed);
    let features = pick_features(species, stage, &mut rng);
    render_lines(&bm, anchors, &features)
}

fn catalog_for(species: SpeciesK) -> PartCatalog {
    match species {
        SpeciesK::Blob => blob_catalog(),
        _ => blob_catalog(), // placeholder until Task 7 fills in the others
    }
}
```

- [ ] **Step 5: Rewrite `main` as Blob-only mini-gallery**

```rust
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
```

- [ ] **Step 6: Run and iterate**

Run: `cargo run --example pet_gallery`
Verify Blob pets render with visible head/body distinction and eye glyphs at correct positions.

If pets look broken: tune the part bitmaps in Steps 1-2 (adjust which pixels are set, eye_anchor positions). This is the aesthetic-tuning loop. Iterate until happy.

- [ ] **Step 7: Tests + commit**

Remove `#[ignore]` from `blueprint_is_deterministic_per_seed` (added in Task 4).

Run: `cargo test --example pet_gallery`

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): Blob species catalog + composer + gallery"
```

### Task 6: Blob accessories (drop-shaped flourishes)

**Files:**
- Modify: `examples/pet_gallery.rs`

Add 3-4 accessory parts to the Blob catalog with `Anchor::HeadTop` or `Anchor::BodySide`. Examples: a small drop on top, a side bump, a bottom drip. Wire them into `compose_parts`.

- [ ] **Step 1: Author accessories**

```rust
mod blob {
    // ... (existing heads and bodies)

    pub static ACC_DROP: Part = Part {
        id: PartId(300),
        rows: &[0b01, 0b11],
        width_px: 2,
        height_px: 2,
        anchor: Anchor::HeadTop,
        min_stage: Stage::S1,
        symmetry: PartSymmetry::Symmetric,
        eye_anchors: None,
    };

    pub static ACC_BUMP: Part = Part {
        id: PartId(301),
        rows: &[0b1],
        width_px: 1,
        height_px: 1,
        anchor: Anchor::BodySide,
        min_stage: Stage::S2,
        symmetry: PartSymmetry::AsymmetricFree,
        eye_anchors: None,
    };

    pub static ACCESSORIES: &[Part] = &[ACC_DROP, ACC_BUMP];

    pub fn catalog() -> PartCatalog {
        PartCatalog {
            heads: HEADS,
            bodies: BODIES,
            accessories: ACCESSORIES,
        }
    }
}
```

- [ ] **Step 2: Extend `compose_parts` to place accessories**

Inside `compose_parts`, after head placement, add:

```rust
for &acc_id in &blueprint.selection.accessories {
    let acc = catalog.accessories.iter().find(|p| p.id == acc_id);
    if let Some(acc) = acc {
        let (ax, ay) = match acc.anchor {
            Anchor::HeadTop => {
                let acc_full_w = if matches!(acc.symmetry, PartSymmetry::HalfMirror) {
                    acc.width_px * 2
                } else { acc.width_px };
                let x = (w.saturating_sub(acc_full_w)) / 2;
                let y = head_y.saturating_sub(acc.height_px);
                (x, y)
            }
            Anchor::BodySide => {
                let x = body_x.saturating_sub(acc.width_px);
                let y = body_y + body.height_px / 2;
                (x, y)
            }
            Anchor::BodyBottom => {
                let x = (w.saturating_sub(acc.width_px)) / 2;
                let y = (body_y + body.height_px).min(h.saturating_sub(acc.height_px));
                (x, y)
            }
            _ => continue,
        };
        render_part(&mut bm, acc, ax, ay);
    }
}
```

- [ ] **Step 3: Run, iterate, commit**

Run: `cargo run --example pet_gallery`
Adjust accessory positions/sizes until they read right.

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): Blob accessory parts + composer accessory placement"
```

### Tasks 7a-7e: Author per-species catalogs

**One task per species. Each follows the same pattern as Tasks 5+6: define ~3 heads, ~3 bodies, ~3-5 accessories, distinct aesthetic per species. Then add the species to `catalog_for`. Aesthetic-iteration-heavy; do inline.**

Each task ends with a commit `feat(pet): <species> species catalog`.

- [ ] **Task 7a: Fuzz catalog** — fluffy/irregular outlines. Heads with edge texture; bodies with whisker/antenna accessories.
- [ ] **Task 7b: Mech catalog** — rectangular/angular. Block bodies, dome/rect heads, piston-arm and tread-foot accessories.
- [ ] **Task 7c: Ghost catalog** — tall, wispy, no defined feet. Drift-veil heads, wave-tail bodies that taper toward the bottom.
- [ ] **Task 7d: Crystal catalog** — faceted/geometric. Triangle/diamond/hex caps; prism/cluster bodies; shoulder-shard accessories.
- [ ] **Task 7e: Glitch catalog** — irregular, scattered. Fragmented heads, scan-line bodies, pixel-scatter artifact accessories.

For each: tune the parts in the gallery before committing. Each species' catalog must produce visibly different macro-shapes from every other species.

### Task 8: Full 50-pet gallery printer

**Files:**
- Modify: `examples/pet_gallery.rs`

- [ ] **Step 1: Replace `main` with the full gallery**

```rust
fn main() {
    println!("parts_gallery — full 50-pet gallery for Phase 0 gate\n");
    let species: [SpeciesK; 6] = [
        SpeciesK::Fuzz, SpeciesK::Blob, SpeciesK::Ghost,
        SpeciesK::Glitch, SpeciesK::Crystal, SpeciesK::Mech,
    ];
    let stages: [Stage; 3] = [Stage::S0, Stage::S1, Stage::S2];

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

- [ ] **Step 2: Run + commit**

```bash
cargo run --example pet_gallery
git add examples/pet_gallery.rs
git commit -m "feat(pet): full 50-pet gallery printer"
```

### Task 9: Phase 0 gate decision

**This is the new gate — replaces Task 22 of plan v1.**

- [ ] **Step 1: Run the gallery**

Run: `cargo run --example pet_gallery | tee /tmp/glorp_pet_gallery_v4.txt`

- [ ] **Step 2: Evaluate against the three updated criteria**

Per the updated spec aesthetic-validation-gate section:

1. **Zero broken pets**: every pet must be a coherent creature (parts compose cleanly, no overlapping artifacts, eye anchors clear).
2. **Species distinctness**: a viewer should be able to identify which species each pet belongs to from macro-shape alone, without reading the label.
3. **≥85% intentional**: most pets read as characterful creatures within their species' aesthetic.

If all three pass → proceed to Task 10 (Phase 1 promotion).

If broken or non-distinct → iterate on the part catalogs (return to Task 5 / 6 / 7a-e as needed) and re-run.

If after sustained iteration the gate cannot pass → escalate to controller for re-scoping (e.g., maybe contract the overhaul to just chrome rework and keep the existing `art.rs`).

- [ ] **Step 3: Commit any final tuning**

```bash
git add examples/pet_gallery.rs
git commit -m "tune(pet): final part catalogs for Phase 0 gate"
```

---

## Phase 1 — Promotion to production

### Task 10: Promote `StableRng` with helpers

Identical to Task 23 of plan v1 — `StableRng` in `src/pet/generation.rs` becomes `pub(crate)` with `next_f32_unit`, `next_bias`, `next_signed_unit` helpers, and `fnv1a64` becomes `pub(crate)`.

**Files:**
- Modify: `src/pet/generation.rs`

- [ ] **Step 1: Add tests for new helpers**

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

- [ ] **Step 2: Run tests (should fail with method-not-found)**

Run: `cargo test --lib pet::generation::stable_rng_tests`

- [ ] **Step 3: Promote `StableRng` and add helpers**

In `src/pet/generation.rs`, change `struct StableRng` and its impl block to:

```rust
#[derive(Debug, Clone)]
pub(crate) struct StableRng { state: u64 }

impl StableRng {
    pub(crate) fn new(seed: u64) -> Self { Self { state: seed.max(1) } }
    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        self.state = x; x
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

Also change `fn fnv1a64` to `pub(crate) fn fnv1a64`.

- [ ] **Step 4: Run tests; commit**

```bash
cargo test --lib pet::generation
git add src/pet/generation.rs
git commit -m "feat(pet): promote StableRng + fnv1a64 with helpers"
```

### Task 11: Create `src/pet/generate.rs` from the spike

**Files:**
- Create: `src/pet/generate.rs`
- Modify: `src/pet/mod.rs`

- [ ] **Step 1: Create the production module**

Copy the working spike code (from `examples/pet_gallery.rs`) into `src/pet/generate.rs`, with these adjustments:

1. Replace `SpikeRng` with `crate::pet::generation::StableRng`. Method names line up (next_u64, next_usize, next_f32_unit, etc.).
2. Replace local `SpeciesK` with the production `crate::pet::generation::Species`.
3. Replace local `Stage` with `crate::game::evolution::Stage`.
4. Drop the `main` function and the gallery-specific code.
5. Keep all part catalogs, the composer, blueprint_for, generate_pet_lines, render_lines, and helpers.
6. Convert the existing tests into a single `#[cfg(test)] mod tests` block at the bottom.

The file should be ~400-600 LOC (parts catalogs are the bulk).

- [ ] **Step 2: Update `src/pet/mod.rs`**

```rust
pub mod art;
pub mod generate;
pub mod generation;
pub mod render;
```

(Keep `art` for now — Task 14 deletes it after render is migrated.)

- [ ] **Step 3: Build the lib**

Run: `cargo build --lib`
Expected: clean build.

- [ ] **Step 4: Add deterministic tests in the new module**

Inside the `tests` module at the bottom of `src/pet/generate.rs`:

```rust
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
    for l in &lines { assert_eq!(l.chars().count(), (w / 2) as usize); }
}
```

- [ ] **Step 5: Run + commit**

```bash
cargo test --lib pet::generate
git add src/pet/generate.rs src/pet/mod.rs
git commit -m "feat(pet): promote parts generator to src/pet/generate.rs"
```

### Task 12: Switch `examples/pet_gallery.rs` to import production code

**Files:**
- Modify: `examples/pet_gallery.rs`

`src/lib.rs` already exposes `pub mod pet`, so the import works.

- [ ] **Step 1: Replace the file with a thin wrapper**

```rust
//! Aesthetic-tuning binary for the production parts generator.

use glorp::game::evolution::Stage;
use glorp::pet::generate::generate_pet_lines;
use glorp::pet::generation::Species;

fn main() {
    println!("parts_gallery — production parts generator\n");
    let species = [
        Species::Fuzz, Species::Blob, Species::Ghost,
        Species::Glitch, Species::Crystal, Species::Mech,
    ];
    let stages: [Stage; 3] = [Stage::S0, Stage::S1, Stage::S2];

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

- [ ] **Step 2: Run + commit**

Run: `cargo run --example pet_gallery`
Expected: 50 pet blocks matching the spike output.

```bash
git add examples/pet_gallery.rs
git commit -m "feat(pet): point pet_gallery example at production parts generator"
```

### Task 13: Rewrite `src/pet/render.rs` against `generate.rs`

**Files:**
- Modify: `src/pet/render.rs`

- [ ] **Step 1: Add failing tests**

Append to `src/pet/render.rs`:

```rust
#[cfg(test)]
mod render_v2_tests {
    use super::*;
    use crate::game::evolution::Stage;
    use crate::game::metabolism::Mood;
    use crate::pet::generation::generate_pet;

    #[test]
    fn render_pet_produces_braille_lines() {
        let pet = generate_pet("test-seed-1");
        let r = render_pet(&pet, Stage::S0, Mood::Content,
                           AnimationFrame { tick: 0, blink_suppression_ticks: 0 });
        assert!(!r.lines.is_empty());
        let has_braille = r.lines.iter().any(|l|
            l.chars().any(|c| (0x2800..=0x28FF).contains(&(c as u32))));
        assert!(has_braille, "expected at least one braille char in rendered lines");
    }

    #[test]
    fn render_pet_deterministic_for_fixed_frame() {
        let pet = generate_pet("test-seed-2");
        let frame = AnimationFrame { tick: 5, blink_suppression_ticks: 0 };
        let a = render_pet(&pet, Stage::S1, Mood::Content, frame);
        let b = render_pet(&pet, Stage::S1, Mood::Content, frame);
        assert_eq!(a.lines, b.lines);
        assert_eq!(a.spans, b.spans);
    }

    #[test]
    fn render_pet_species_differ_at_s2() {
        // Two pets of same seed but synthetic different species would produce
        // different outputs. Use two real pets via generate_pet to ensure
        // species differ. This is a smoke test that species reach the renderer.
        let pet_a = generate_pet("seed-aaa");
        let pet_b = generate_pet("seed-bbb");
        if pet_a.species == pet_b.species { return; } // skip if same species drawn
        let frame = AnimationFrame { tick: 0, blink_suppression_ticks: 0 };
        let a = render_pet(&pet_a, Stage::S2, Mood::Content, frame);
        let b = render_pet(&pet_b, Stage::S2, Mood::Content, frame);
        assert_ne!(a.lines, b.lines);
    }
}
```

- [ ] **Step 2: Run tests (expected to fail — render_pet still uses art.rs)**

Run: `cargo test --lib pet::render::render_v2_tests`
Expected: FAIL.

- [ ] **Step 3: Replace `render_pet`'s body**

In `src/pet/render.rs`, replace the existing `render_pet` function body with:

```rust
pub fn render_pet(
    pet: &GeneratedPet,
    stage: Stage,
    mood: Mood,
    frame: AnimationFrame,
) -> RenderedPet {
    use crate::pet::generate::generate_pet_lines;
    use crate::pet::generation::fnv1a64;

    let _seed = fnv1a64(&pet.seed);
    let lines = generate_pet_lines(pet.species, stage, fnv1a64(&pet.seed));

    // Build StyledSegments for every cell. Heuristic: braille cells (0x2800-0x28FF)
    // are body; other cells are accent or feature glyphs.
    //
    // The eye/mouth glyph swap for blink/mood is applied here on top of the
    // generator output, by replacing characters at the head's known eye row.
    // For Phase 1 we accept that mood-driven eye/mouth glyph swap is *not yet*
    // applied — it lands with Phase 3 (PetAnimator). Today the renderer just
    // returns whatever the parts generator produced.
    let mut spans = Vec::new();
    for (line_idx, line) in lines.iter().enumerate() {
        for (ch_idx, c) in line.chars().enumerate() {
            let role = if (0x2800..=0x28FF).contains(&(c as u32)) {
                PaletteRoleName::Body
            } else {
                // Feature glyph (eye or mouth). Use a simple heuristic for
                // line position to assign role.
                if line_idx == 0 { PaletteRoleName::Eye }
                else { PaletteRoleName::Mouth }
            };
            spans.push(StyledSegment { line: line_idx, start: ch_idx, end: ch_idx + 1, role });
        }
    }

    // TODO(Phase 3): mood-driven eye/mouth swap + blink suppression. For now
    // we ignore `mood` and `frame` parameters (they're consumed by the
    // animator in Phase 3).
    let _ = (mood, frame);

    RenderedPet { lines, spans, event_lines: Vec::new() }
}
```

Note: `_seed` is just to verify the import; remove if not used. Also delete the old helper functions in `render.rs` that referenced `template_lines`, `expression_for`, `stage_key`, etc. — `cargo build` will surface them.

- [ ] **Step 4: Build, fix import errors**

Run: `cargo build --lib`
Iterate until clean (likely removing now-orphan helpers and `use pet::art::*` import at the top of `render.rs`).

- [ ] **Step 5: Update old render.rs tests**

The existing tests in `render.rs` likely assert specific characters or widths from the old `art.rs` templates. Either delete those tests (the new `render_v2_tests` cover the new behavior) or rewrite them against the new braille output.

- [ ] **Step 6: Run + commit**

```bash
cargo test --lib pet
git add src/pet/render.rs
git commit -m "feat(pet): rewire render.rs to consume parts generator"
```

### Task 14: Delete `src/pet/art.rs` and migrate stragglers

**Files:**
- Delete: `src/pet/art.rs`
- Modify: `src/pet/mod.rs`
- Possibly modify: any other file that imported `pet::art::*`

- [ ] **Step 1: Find remaining callers**

Run: `grep -rn "pet::art\|pet/art\|use crate::pet::art\|use glorp::pet::art" src/ examples/ tests/`

For each match outside `src/pet/mod.rs`: migrate the caller to use `pet::generate::generate_pet_lines` (or the appropriate alternative).

- [ ] **Step 2: Remove module declaration**

Edit `src/pet/mod.rs`:

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

- [ ] **Step 5: Run full test suite**

Run: `cargo test`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add -u
git commit -m "feat(pet): delete art.rs; templates fully replaced by parts generator"
```

### Task 15: End-to-end smoke test — `glorp watch`

**Files:** none modified (unless smoke test fails)

- [ ] **Step 1: Build binary**

Run: `cargo build --bin glorp`
Expected: clean build.

- [ ] **Step 2: Run watch mode**

Run: `cargo run --bin glorp -- watch` (if a glorp state exists; otherwise `glorp init` first).

Verify the pet panel renders parts-based pets. Press `q` to exit.

- [ ] **Step 3: Run lints**

Run: `cargo clippy --all-targets -- -D warnings`

- [ ] **Step 4: Commit any clippy fixes**

If needed:
```bash
git add -u
git commit -m "chore: clippy fixes after parts migration"
```

### Task 16: Insta snapshot tests for the generator

**Files:**
- Modify: `src/pet/generate.rs`

- [ ] **Step 1: Add snapshot tests**

Append to `src/pet/generate.rs` tests module:

```rust
#[test]
fn snapshot_blob_s0() {
    insta::assert_snapshot!(generate_pet_lines(Species::Blob, Stage::S0, 42).join("\n"));
}

#[test]
fn snapshot_mech_s2() {
    insta::assert_snapshot!(generate_pet_lines(Species::Mech, Stage::S2, 99).join("\n"));
}

#[test]
fn snapshot_ghost_s1() {
    insta::assert_snapshot!(generate_pet_lines(Species::Ghost, Stage::S1, 137).join("\n"));
}

#[test]
fn snapshot_glitch_s2() {
    insta::assert_snapshot!(generate_pet_lines(Species::Glitch, Stage::S2, 21).join("\n"));
}
```

- [ ] **Step 2: Review and accept snapshots**

Run: `cargo test --lib pet::generate::tests::snapshot_`
Then: `cargo insta review` (install with `cargo install cargo-insta` if needed).

- [ ] **Step 3: Re-run + commit**

```bash
cargo test --lib pet::generate
git add src/pet/generate.rs src/pet/snapshots/
git commit -m "test(pet): lock parts generator output via insta snapshots"
```

### Task 17: Final regression sweep

- [ ] **Step 1: Full test suite**

Run: `cargo test`

- [ ] **Step 2: Clippy**

Run: `cargo clippy --all-targets -- -D warnings`

- [ ] **Step 3: Format**

Run: `cargo fmt --all -- --check`
If anything's off: `cargo fmt --all` and commit.

- [ ] **Step 4: Verify LOC**

Run: `git diff --stat $(git log --oneline --grep="reset spike to parts" | head -1 | cut -d' ' -f1)^ HEAD -- src/pet/ examples/pet_gallery.rs`

Expected: substantial net deletion from src/pet/art.rs (~630 LOC) offset by additions in pet/generate.rs.

- [ ] **Step 5: Final commit**

```bash
git add -u
git commit -m "chore: final cleanup after parts migration" || true
```

---

## Summary

This plan delivers:
- A new compositional-parts pet generator with species-specific catalogs.
- Procedural composition: per-seed selection of head + body + accessories from each species' catalog.
- Production module `src/pet/generate.rs` replacing `src/pet/art.rs`.
- `src/pet/render.rs` rewired to consume the new generator.
- Insta snapshots locking the tuned output.

Phases 2–6 (layout, animator, tachyonfx, mouse, polish) follow in separate plans.
