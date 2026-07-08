# Glorp Pixel Cast Identity And Tank Composition Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the opt-in Pixel companion read as the real six-species Glorp cast by promoting canonical pet-art cues into renderer-visible roles, exporting privacy-safe review artifacts, and proving those roles in Preview Lab without changing the default renderer.

**Architecture:** Keep canonical terminal art owned by `presentation::pixel::art_reference`; expose sanitized Pixel reference cells, protected regions, and cue coverage from there. The Pixel renderer consumes those roles directly, while Preview Lab records schema-versioned evidence and composition checks without mutating live prop or tank-life placement behavior.

**Tech Stack:** Rust 2021, existing `serde` / `serde_json`, existing Preview Lab exporter, existing `ratatui` frame capture, existing Pixel renderer and `round::pixel_fit` helper.

## Global Constraints

- Pixel remains opt-in and Classic remains available.
- Do not remove Classic.
- Do not flip Pixel to the default renderer.
- Do not add a hand-authored sprite sheet or external asset pipeline.
- Do not add new tank-life unlocks, inhabitants, routes, prop catalog entries, or prop mechanics.
- Do not add a live Pixel tank compositor or runtime prop/tank-life avoidance system.
- Do not redesign the round tank composition.
- Keep `presentation::pixel::art_reference` as the only canonical terminal-art adapter for typed Pixel contracts.
- Preview Lab must consume sanitized `PixelPetArtReference` output and must not become a second canonical-art extractor for typed artifacts.
- `PixelArtCell` remains a single exclusive render role unless implementation evidence proves an additive role model is necessary.
- Exported review artifacts must not expose raw seeds, raw terminal art, source names, usage counts, absolute/user filesystem paths, diagnostics, prompts, responses, or transcripts.
- Relative artifact paths such as `frames/*.json` are allowed because they are the Preview manifest contract.
- Pixel Preview `.txt`, `.cells.json`, and HTML outputs must omit canonical terminal reference rows in the default privacy-safe review contract.
- Product catalog prop/tank-life identifiers and glyph cells may appear only in fixture composition evidence and must not be treated as user/source identifiers.
- Add no new dependencies.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `src/presentation/pixel/art_reference.rs` | Promote signature cue cells into exclusive `PixelArtRole`s, add protected-region and cue-coverage data, and keep raw terminal glyphs private. |
| `src/presentation/pixel/mod.rs` | Export new Pixel reference contract types. |
| `src/presentation/pixel/animator.rs` | Make promoted roles visibly distinct enough that renderer tests can prove they are not metadata-only. |
| `src/dev_preview/export.rs` | Bump Pixel art sidecar to schema `2`; add Pixel composition artifact schema, manifest file slot, path, and artifact type. |
| `src/dev_preview/contract.rs` | Carry `pixel_composition` on `PreviewFrameContract`. |
| `src/dev_preview/scenarios.rs` | Write composition artifacts and list them in manifest files/artifacts. |
| `src/dev_preview/pixel.rs` | Remove default terminal-reference rows, emit Pixel art schema `2`, add six cast fixtures, matrix grouping frame, and composition evidence frame. |
| `tests/pixel_art_reference.rs` | Prove cue promotion, protected regions, cue coverage, cache stability, and privacy. |
| `tests/pixel_renderer.rs` | Prove promoted roles affect pixels and preserve all-species smoke coverage. |
| `tests/dev_preview.rs` | Prove schema `2`, cast fixture IDs, matrix grouping, composition artifacts, and privacy allowlist coverage. |
| `tests/pixel_fit.rs` | Keep existing HUD-safe geometry coverage green. |
| `docs/superpowers/measurements/2026-07-08-glorp-pixel-cast-identity-tank-composition-review.md` | Record generated Preview evidence and manual review status after implementation. |

---

### Task 1: Promote Canonical Cue Cells In `PixelPetArtReference`

**Files:**
- Modify: `src/presentation/pixel/art_reference.rs`
- Modify: `src/presentation/pixel/mod.rs`
- Test: `tests/pixel_art_reference.rs`

**Interfaces:**
- Produces: `PixelProtectedRegion { id: &'static str, role: &'static str, bounds: PixelCellBounds, cell_count: usize }`
- Produces: `PixelCueCoverage { expected: usize, present: usize }`
- Extends: `PixelPetArtReference { protected_regions: Vec<PixelProtectedRegion>, cue_coverage: BTreeMap<&'static str, PixelCueCoverage> }`
- Extends: `PixelPetArtReference::protected_region(&self, id: &str) -> Option<PixelProtectedRegion>`
- Extends: `PixelPetArtReference::cue_coverage(&self, id: &str) -> Option<PixelCueCoverage>`

- [ ] **Step 1: Write failing role-promotion tests**

Append these tests to `tests/pixel_art_reference.rs`:

```rust
#[test]
fn fuzz_s3_promotes_locket_cells_into_visible_roles() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;
    vm.pet_render.mood = Mood::Content;

    let reference = reference_for(&vm, 0);
    let locket_cells = reference.cells_for_roles([PixelArtRole::Locket]);
    let coverage = reference.cue_coverage("locket").expect("locket coverage");

    assert!(!locket_cells.is_empty(), "locket cells must be promoted");
    assert_eq!(coverage.expected, coverage.present);
    assert!(coverage.present >= 1);
    assert!(reference.protected_region("signature-locket").is_some());
}

#[test]
fn crystal_s5_promotes_facet_cells_into_visible_roles() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Crystal;
    vm.pet_render.stage = Stage::S5;
    vm.pet_render.mood = Mood::Happy;

    let reference = reference_for(&vm, 0);
    let facet_cells = reference.cells_for_roles([PixelArtRole::Facet]);
    let coverage = reference.cue_coverage("facet").expect("facet coverage");

    assert!(!facet_cells.is_empty(), "facet cells must be promoted");
    assert_eq!(coverage.expected, coverage.present);
    assert!(coverage.present >= 1);
    assert!(reference.protected_region("signature-facet").is_some());
}

#[test]
fn glitch_s4_promotes_repair_cells_without_stealing_face_cells() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Glitch;
    vm.pet_render.stage = Stage::S4;
    vm.pet_render.mood = Mood::Content;
    vm.life_profile.burst_level = 0.9;
    vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(300));

    let reference = reference_for(&vm, 300);
    let repair_cells = reference.cells_for_roles([PixelArtRole::RepairMark]);
    let face_cells = reference.cells_for_roles([PixelArtRole::Eye, PixelArtRole::Mouth]);
    let coverage = reference.cue_coverage("repair_mark").expect("repair coverage");

    assert!(!repair_cells.is_empty(), "repair cells must be promoted");
    assert_eq!(coverage.expected, coverage.present);
    assert!(face_cells.iter().all(|cell| !repair_cells.contains(cell)));
    assert!(reference.protected_region("face").is_some());
    assert!(reference.protected_region("signature-repair-mark").is_some());
}

#[test]
fn outline_appendage_and_foot_contact_are_promoted_cells_not_counts_only() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Mech;
    vm.pet_render.stage = Stage::S5;
    vm.pet_render.mood = Mood::Content;

    let reference = reference_for(&vm, 0);

    assert!(!reference.cells_for_roles([PixelArtRole::Outline]).is_empty());
    assert!(!reference.cells_for_roles([PixelArtRole::Appendage]).is_empty());
    assert!(!reference.cells_for_roles([PixelArtRole::FootContact]).is_empty());
    assert_eq!(
        reference.role_count(PixelArtRole::FootContact),
        reference.foot_contact.cells.len()
    );
}

#[test]
fn serialized_reference_exports_sanitized_protected_regions_and_cue_coverage() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.seed = "very-secret-seed".to_string();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;

    let reference = reference_for(&vm, 0);
    let json = serde_json::to_string(&reference).unwrap();

    assert!(json.contains("\"protected_regions\""));
    assert!(json.contains("\"cue_coverage\""));
    assert!(json.contains("signature-locket"));
    assert!(!json.contains("very-secret-seed"));
    assert!(!json.contains("terminal"));
    assert!(!json.contains("art_text"));
}
```

- [ ] **Step 2: Run tests to verify the new assertions fail**

Run:

```bash
cargo test --test pixel_art_reference -- --nocapture
```

Expected: FAIL because `protected_region` and `cue_coverage` do not exist, and promoted cells such as `Locket`, `Facet`, `RepairMark`, `Outline`, `Appendage`, and `FootContact` are still mostly aggregate counts.

- [ ] **Step 3: Add reference contract types**

In `src/presentation/pixel/art_reference.rs`, add these types near `PixelFootContact`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelProtectedRegion {
    pub id: &'static str,
    pub role: &'static str,
    pub bounds: PixelCellBounds,
    pub cell_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PixelCueCoverage {
    pub expected: usize,
    pub present: usize,
}
```

Extend `PixelPetArtReference`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PixelPetArtReference {
    pub species: Species,
    pub stage: Stage,
    pub mood: Mood,
    pub pose: PixelArtPoseKey,
    pub width_cells: u8,
    pub height_cells: u8,
    pub occupied_cells: Vec<PixelArtCell>,
    pub body_bounds: PixelCellBounds,
    pub foot_contact: PixelFootContact,
    pub protected_regions: Vec<PixelProtectedRegion>,
    pub cue_coverage: BTreeMap<&'static str, PixelCueCoverage>,
    pub reference_checksum: PixelReferenceChecksum,
    pub role_counts: BTreeMap<&'static str, usize>,
}
```

Add these methods to `impl PixelPetArtReference`:

```rust
pub fn protected_region(&self, id: &str) -> Option<PixelProtectedRegion> {
    self.protected_regions
        .iter()
        .copied()
        .find(|region| region.id == id)
}

pub fn cue_coverage(&self, id: &str) -> Option<PixelCueCoverage> {
    self.cue_coverage.get(id).copied()
}
```

In `src/presentation/pixel/mod.rs`, export the new types:

```rust
pub use art_reference::{
    PixelArtCell, PixelArtPoseKey, PixelArtReferenceProvider, PixelArtReferenceRequest,
    PixelArtRole, PixelCanonicalAnimationInputs, PixelCellBounds, PixelCueCoverage,
    PixelFootContact, PixelPetArtReference, PixelProtectedRegion, PixelReferenceChecksum,
};
```

- [ ] **Step 4: Promote roles during reference extraction**

In `render_reference`, keep the first pass that builds base `occupied_cells`, then compute `structural_cells`, `foot_contact`, and `footprint`. Replace the current `count_overlay_roles` call with a promotion pass:

```rust
let base_occupied_cells = occupied_cells.clone();
let mut occupied_cells = promote_reference_cells(
    request,
    &rendered.lines,
    occupied_cells,
    &footprint,
    &foot_contact,
);
occupied_cells.sort_by_key(|cell| (cell.y, cell.x, cell.role.as_str()));

let role_counts = role_counts_for(&occupied_cells);
let cue_coverage =
    cue_coverage_for(request, &rendered.lines, &base_occupied_cells, &occupied_cells);
let protected_regions = protected_regions_for(&occupied_cells);
```

Add these helper functions in `art_reference.rs`:

```rust
fn promote_reference_cells(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    cells: Vec<PixelArtCell>,
    footprint: &BTreeSet<(u8, u8)>,
    foot_contact: &PixelFootContact,
) -> Vec<PixelArtCell> {
    cells
        .into_iter()
        .map(|mut cell| {
            if cell.role == PixelArtRole::Particle {
                return cell;
            }
            if matches!(cell.role, PixelArtRole::Eye | PixelArtRole::Mouth) {
                return cell;
            }

            let glyph = glyph_at(lines, cell.x, cell.y);
            cell.role = promoted_role_for(request, cell, glyph, footprint, foot_contact);
            cell
        })
        .collect()
}

fn promoted_role_for(
    request: &PixelArtReferenceRequest,
    cell: PixelArtCell,
    glyph: char,
    footprint: &BTreeSet<(u8, u8)>,
    foot_contact: &PixelFootContact,
) -> PixelArtRole {
    if request.species == Species::Fuzz && LOCKET_GLYPHS.contains(&glyph) {
        return PixelArtRole::Locket;
    }
    if request.species == Species::Crystal && FACET_GLYPHS.contains(&glyph) {
        return PixelArtRole::Facet;
    }
    if request.species == Species::Glitch
        && matches!(request.stage, Stage::S4 | Stage::S5 | Stage::S6)
        && matches!(cell.role, PixelArtRole::Accent | PixelArtRole::Pattern)
        && GLITCH_REPAIR_GLYPHS.contains(&glyph)
    {
        return PixelArtRole::RepairMark;
    }
    if foot_contact.cells.contains(&(cell.x, cell.y)) {
        return PixelArtRole::FootContact;
    }
    if is_appendage_cell(&cell, footprint) {
        return PixelArtRole::Appendage;
    }
    if matches!(cell.role, PixelArtRole::Body | PixelArtRole::BodyGlow) {
        if is_outline_cell(cell.x, cell.y, footprint) {
            return PixelArtRole::Outline;
        }
        return PixelArtRole::InteriorTexture;
    }
    cell.role
}

fn role_counts_for(cells: &[PixelArtCell]) -> BTreeMap<&'static str, usize> {
    let mut role_counts = BTreeMap::new();
    for cell in cells {
        *role_counts.entry(cell.role.as_str()).or_insert(0) += 1;
    }
    role_counts
}
```

Add cue coverage and protected-region helpers:

```rust
fn cue_coverage_for(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    base_cells: &[PixelArtCell],
    promoted_cells: &[PixelArtCell],
) -> BTreeMap<&'static str, PixelCueCoverage> {
    let mut coverage = BTreeMap::new();
    coverage.insert(
        "locket",
        coverage_for_glyph_role(request, lines, base_cells, promoted_cells, Species::Fuzz, &LOCKET_GLYPHS, PixelArtRole::Locket),
    );
    coverage.insert(
        "facet",
        coverage_for_glyph_role(request, lines, base_cells, promoted_cells, Species::Crystal, &FACET_GLYPHS, PixelArtRole::Facet),
    );
    coverage.insert(
        "repair_mark",
        glitch_repair_coverage(request, lines, base_cells, promoted_cells),
    );
    coverage
}

fn coverage_for_glyph_role(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    base_cells: &[PixelArtCell],
    promoted_cells: &[PixelArtCell],
    species: Species,
    glyphs: &[char],
    role: PixelArtRole,
) -> PixelCueCoverage {
    if request.species != species {
        return PixelCueCoverage { expected: 0, present: 0 };
    }
    let expected = base_cells
        .iter()
        .filter(|cell| glyphs.contains(&glyph_at(lines, cell.x, cell.y)))
        .count();
    let present = promoted_cells.iter().filter(|cell| cell.role == role).count();
    PixelCueCoverage { expected, present }
}

fn glitch_repair_coverage(
    request: &PixelArtReferenceRequest,
    lines: &[String],
    base_cells: &[PixelArtCell],
    promoted_cells: &[PixelArtCell],
) -> PixelCueCoverage {
    if request.species != Species::Glitch || !matches!(request.stage, Stage::S4 | Stage::S5 | Stage::S6) {
        return PixelCueCoverage { expected: 0, present: 0 };
    }
    let expected = base_cells
        .iter()
        .filter(|cell| {
            matches!(
                cell.role,
                PixelArtRole::Accent | PixelArtRole::Pattern
            ) && GLITCH_REPAIR_GLYPHS.contains(&glyph_at(lines, cell.x, cell.y))
        })
        .count();
    let present = promoted_cells
        .iter()
        .filter(|cell| cell.role == PixelArtRole::RepairMark)
        .count();
    PixelCueCoverage { expected, present }
}

fn protected_regions_for(cells: &[PixelArtCell]) -> Vec<PixelProtectedRegion> {
    let groups: [(&str, &str, &[PixelArtRole]); 4] = [
        ("face", "face", &[PixelArtRole::Eye, PixelArtRole::Mouth]),
        ("signature-locket", "signature", &[PixelArtRole::Locket]),
        ("signature-facet", "signature", &[PixelArtRole::Facet]),
        ("signature-repair-mark", "signature", &[PixelArtRole::RepairMark]),
    ];
    groups
        .into_iter()
        .filter_map(|(id, role, roles)| protected_region_for_roles(id, role, cells, roles))
        .collect()
}

fn protected_region_for_roles(
    id: &'static str,
    role: &'static str,
    cells: &[PixelArtCell],
    roles: &[PixelArtRole],
) -> Option<PixelProtectedRegion> {
    let matching = cells
        .iter()
        .copied()
        .filter(|cell| roles.contains(&cell.role))
        .collect::<Vec<_>>();
    let bounds = bounds_for(&matching)?;
    Some(PixelProtectedRegion {
        id,
        role,
        bounds,
        cell_count: matching.len(),
    })
}
```

Keep `count_overlay_roles` only if another test still needs it; otherwise remove it once all callers are gone.

- [ ] **Step 5: Populate new fields and keep checksum stable for role changes**

Update the `PixelPetArtReference` construction in `render_reference`:

```rust
PixelPetArtReference {
    species: request.species,
    stage: request.stage,
    mood: request.mood,
    pose: request.pose,
    width_cells,
    height_cells,
    occupied_cells,
    body_bounds,
    foot_contact,
    protected_regions,
    cue_coverage,
    reference_checksum,
    role_counts,
}
```

Leave `reference_checksum(request, &occupied_cells, body_bounds)` after promotion so promoted roles are part of the checksum.

- [ ] **Step 6: Update fallback test references**

In `tests/pixel_renderer.rs`, any manually constructed `PixelPetArtReference` must add:

```rust
protected_regions: Vec::new(),
cue_coverage: std::collections::BTreeMap::new(),
```

- [ ] **Step 7: Run role-reference tests**

Run:

```bash
cargo test --test pixel_art_reference -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit Task 1**

```bash
git add src/presentation/pixel/art_reference.rs src/presentation/pixel/mod.rs tests/pixel_art_reference.rs tests/pixel_renderer.rs
git commit -m "feat(pixel): promote cast identity roles"
```

---

### Task 2: Make Promoted Roles Visibly Affect Pixel Rendering

**Files:**
- Modify: `src/presentation/pixel/animator.rs`
- Test: `tests/pixel_renderer.rs`

**Interfaces:**
- Consumes: `PixelPetArtReference.occupied_cells` with promoted exclusive roles from Task 1.
- Produces: visible color differences for `Locket`, `Facet`, `RepairMark`, `Outline`, `InteriorTexture`, `Appendage`, and `FootContact`.

- [ ] **Step 1: Write failing renderer-impact tests**

Append these helper functions and tests to `tests/pixel_renderer.rs`:

```rust
fn frame_for_reference(vm: &WatchViewModel, reference: PixelPetArtReference) -> PixelFrame {
    let base = datetime!(2026-07-08 12:00 UTC);
    let input = PixelPetInput::from_watch_view_model(vm, base);
    let mut state = PixelRendererState::new(&input, base);
    render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &reference,
        viewport: PixelViewport::companion_default(),
        now: base,
        state: &mut state,
    })
}

fn reference_with_role_change(
    vm: &WatchViewModel,
    from: PixelArtRole,
    to: PixelArtRole,
) -> (PixelPetArtReference, PixelPetArtReference) {
    let (_frame, base_reference) = frame_for_with_reference(vm, 0);
    let mut changed = base_reference.clone();
    let cell = changed
        .occupied_cells
        .iter_mut()
        .find(|cell| cell.role == from)
        .expect("reference should contain source role");
    cell.role = to;
    (base_reference, changed)
}

#[test]
fn signature_roles_change_visible_pixels() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;

    let (base_reference, locket_reference) =
        reference_with_role_change(&vm, PixelArtRole::Body, PixelArtRole::Locket);
    let base_frame = frame_for_reference(&vm, base_reference);
    let locket_frame = frame_for_reference(&vm, locket_reference);

    assert!(
        base_frame.changed_pixel_count(&locket_frame) > 0,
        "locket role must change visible pixels"
    );
}

#[test]
fn structural_roles_change_visible_pixels() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Mech;
    vm.pet_render.stage = Stage::S5;

    let (base_reference, outline_reference) =
        reference_with_role_change(&vm, PixelArtRole::Body, PixelArtRole::Outline);
    let base_frame = frame_for_reference(&vm, base_reference);
    let outline_frame = frame_for_reference(&vm, outline_reference);

    assert!(
        base_frame.changed_pixel_count(&outline_frame) > 0,
        "outline role must change visible pixels"
    );
}

#[test]
fn promoted_reference_roles_are_visible_in_hero_frames() {
    for (species, stage, required_role) in [
        (Species::Fuzz, Stage::S3, PixelArtRole::Locket),
        (Species::Glitch, Stage::S4, PixelArtRole::RepairMark),
        (Species::Crystal, Stage::S5, PixelArtRole::Facet),
        (Species::Mech, Stage::S5, PixelArtRole::Outline),
    ] {
        let mut vm = WatchViewModel::fixture();
        vm.pet_render.generated_species = species;
        vm.pet_render.stage = stage;
        let (frame, reference) = frame_for_with_reference(&vm, 480);

        assert!(
            reference.role_count(required_role) > 0,
            "{species:?} {stage:?} missing required promoted role {required_role:?}"
        );
        assert!(
            frame.opaque_pixel_count() > 120,
            "{species:?} {stage:?} rendered too few visible pixels"
        );
    }
}
```

- [ ] **Step 2: Run renderer tests to verify impact failures**

Run:

```bash
cargo test --test pixel_renderer signature_roles_change_visible_pixels structural_roles_change_visible_pixels promoted_reference_roles_are_visible_in_hero_frames -- --nocapture
```

Expected: FAIL before renderer role colors are distinct enough, or before Task 1 has promoted hero roles.

- [ ] **Step 3: Make structural roles visually distinct**

In `src/presentation/pixel/animator.rs`, add this helper near `rgba_with_alpha`:

```rust
fn rgba_scaled(rgb: crate::pet::palette::Rgb, scale: f32) -> Rgba8 {
    let scale = scale.clamp(0.0, 1.0);
    Rgba8::opaque(
        (f32::from(rgb.r) * scale).round() as u8,
        (f32::from(rgb.g) * scale).round() as u8,
        (f32::from(rgb.b) * scale).round() as u8,
    )
}
```

Replace `color_for_role` with:

```rust
fn color_for_role(input: &PixelPetInput, role: PixelArtRole) -> Rgba8 {
    match role {
        PixelArtRole::Eye | PixelArtRole::Mouth => rgba_opaque(input.palette.eye),
        PixelArtRole::Corruption => rgba_opaque(input.palette.corruption),
        PixelArtRole::Pattern => rgba_opaque(input.palette.pattern),
        PixelArtRole::Accent | PixelArtRole::Particle => rgba_opaque(input.palette.accent),
        PixelArtRole::Locket | PixelArtRole::Facet | PixelArtRole::RepairMark => {
            rgba_opaque(input.palette.accent)
        }
        PixelArtRole::Outline => rgba_scaled(input.palette.body, 0.62),
        PixelArtRole::InteriorTexture => rgba_scaled(input.palette.body, 0.84),
        PixelArtRole::Appendage => rgba_scaled(input.palette.body, 0.92),
        PixelArtRole::FootContact => rgba_scaled(input.palette.body, 0.72),
        PixelArtRole::Body | PixelArtRole::BodyGlow => rgba_opaque(input.palette.body),
    }
}
```

- [ ] **Step 4: Run renderer tests**

Run:

```bash
cargo test --test pixel_renderer -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit Task 2**

```bash
git add src/presentation/pixel/animator.rs tests/pixel_renderer.rs
git commit -m "feat(pixel): render promoted identity roles"
```

---

### Task 3: Add Pixel Art Schema 2 And Privacy-Safe Preview Artifacts

**Files:**
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/contract.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `src/dev_preview/pixel.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Produces: `PIXEL_ART_SCHEMA_VERSION: u32 = 2`
- Produces: `PIXEL_COMPOSITION_SCHEMA_VERSION: u32 = 1`
- Produces: `PreviewPixelArtCellArtifact`
- Produces: `PreviewPixelProtectedRegionArtifact`
- Produces: `PreviewPixelCueCoverageArtifact`
- Produces: `PreviewPixelCompositionArtifact`
- Extends: `PreviewScenarioFiles.pixel_composition: Option<PathBuf>`
- Extends: `PreviewFrameContract.pixel_composition: Option<PreviewPixelCompositionArtifact>`
- Produces path format: `frames/<id>.pixel-composition.json`

- [ ] **Step 1: Write failing Preview schema and privacy tests**

Update `tests/dev_preview.rs`:

1. Extend `collect_pixel_review_artifact_paths` so it includes `name.ends_with(".pixel-composition.json")`.
2. In `dev_preview_pixel_writes_art_and_fit_sidecars`, replace the art sidecar assertions with:

```rust
let art: Value = serde_json::from_str(&art_json).unwrap();
assert_eq!(art["schema_version"], 2);
assert!(art["role_cells"].as_array().unwrap().len() > 20);
assert!(art["protected_bounds"].as_array().unwrap().iter().any(|region| {
    region["id"] == "face"
}));
assert!(art["cue_coverage"].as_object().unwrap().contains_key("locket"));
assert!(art["signature_regions"].as_array().unwrap().iter().any(|region| {
    region["id"].as_str().is_some_and(|id| id.starts_with("signature-"))
}));
assert!(!art_json.contains("fixture-seed"));
assert!(!art_json.contains("art_text"));
```

3. Add this test:

```rust
#[test]
fn dev_preview_pixel_privacy_outputs_omit_terminal_reference_rows() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let text =
        std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.txt")).unwrap();
    let cells =
        std::fs::read_to_string(run.out.join("frames/pixel-fuzz-s3-content-idle.cells.json"))
            .unwrap();
    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();

    for content in [&text, &cells, &html] {
        assert!(!content.contains("terminal reference"));
        assert!(!content.contains("/\\_/\\"));
        assert!(!content.contains("( o.o )"));
        assert!(!content.contains("very-secret-seed"));
    }
}
```

4. Add this test:

```rust
#[test]
fn dev_preview_pixel_composition_artifact_has_own_manifest_slot() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let scenario = scenario(&manifest, "pixel-tank-composition");

    assert_eq!(
        scenario["files"]["pixel_composition"],
        "frames/pixel-tank-composition.pixel-composition.json"
    );
    assert_artifact_type(
        &manifest,
        "pixel-tank-composition-pixel-composition",
        "pixel-composition",
    );

    let composition = run.read_json("frames/pixel-tank-composition.pixel-composition.json");
    assert_eq!(composition["schema_version"], 1);
    assert_eq!(composition["frame_id"], "pixel-tank-composition");
    assert!(composition["protected_regions"].as_array().unwrap().iter().any(|region| {
        region["id"] == "face"
    }));
    assert!(composition["context"]["surface"].is_string());
}
```

- [ ] **Step 2: Run Preview tests to verify failures**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_pixel_writes_art_and_fit_sidecars dev_preview_pixel_privacy_outputs_omit_terminal_reference_rows dev_preview_pixel_composition_artifact_has_own_manifest_slot -- --nocapture
```

Expected: FAIL because schema `2`, composition files, and terminal-reference redaction are not implemented.

- [ ] **Step 3: Extend export artifact types**

In `src/dev_preview/export.rs`, change constants and imports:

```rust
use crate::presentation::pixel::{
    PixelCellBounds, PixelCueCoverage, PixelFootContact, PixelProtectedRegion,
};

pub const PIXEL_ART_SCHEMA_VERSION: u32 = 2;
pub const PIXEL_COMPOSITION_SCHEMA_VERSION: u32 = 1;
```

Extend `PreviewScenarioFiles`:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub pixel_composition: Option<PathBuf>,
```

Replace `PreviewPixelArtArtifact` with:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelArtArtifact {
    pub schema_version: u32,
    pub species: String,
    pub stage: String,
    pub mood: String,
    pub reference_checksum: String,
    pub width_cells: u8,
    pub height_cells: u8,
    pub body_bounds: PixelCellBounds,
    pub foot_contact: PixelFootContact,
    pub role_counts: BTreeMap<&'static str, usize>,
    pub role_cells: Vec<PreviewPixelArtCellArtifact>,
    pub protected_bounds: Vec<PreviewPixelProtectedRegionArtifact>,
    pub signature_regions: Vec<PreviewPixelProtectedRegionArtifact>,
    pub cue_coverage: BTreeMap<&'static str, PixelCueCoverage>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelArtCellArtifact {
    pub x: u8,
    pub y: u8,
    pub role: &'static str,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelProtectedRegionArtifact {
    pub id: &'static str,
    pub role: &'static str,
    pub bounds: PixelCellBounds,
    pub cell_count: usize,
}
```

Add composition structs:

```rust
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelCompositionArtifact {
    pub schema_version: u32,
    pub frame_id: String,
    pub context: PreviewPixelCompositionContextArtifact,
    pub protected_regions: Vec<PreviewPixelProtectedRegionArtifact>,
    pub comparison: PreviewPixelCompositionComparisonArtifact,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelCompositionContextArtifact {
    pub surface: String,
    pub props_available: bool,
    pub tank_life_available: bool,
    pub evidence_mode: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewPixelCompositionComparisonArtifact {
    pub protected_region_count: usize,
    pub prop_cells_near_protected_regions: usize,
    pub tank_life_cells_near_protected_regions: usize,
    pub occlusion_conflicts: Vec<String>,
    pub deferred_contexts: Vec<String>,
}
```

Extend `ArtifactType`:

```rust
PixelComposition,
```

- [ ] **Step 4: Carry composition through frame contracts and manifest paths**

In `src/dev_preview/contract.rs`, add:

```rust
pub pixel_composition: Option<crate::dev_preview::export::PreviewPixelCompositionArtifact>,
```

to `PreviewFrameContract`.

In `src/dev_preview/scenarios.rs`:

1. Import no new crate dependencies.
2. In the frame artifact write loop, add:

```rust
if let Some(pixel_composition) = &frame.contract.pixel_composition {
    write_json_artifact(
        &staging_dir.join(pixel_composition_path(frame)),
        pixel_composition,
    )?;
}
```

3. In `scenario_from_parts`, set:

```rust
pixel_composition: frame
    .contract
    .pixel_composition
    .as_ref()
    .map(|_| pixel_composition_path(frame)),
```

4. In `artifacts_for_frames`, add:

```rust
if frame.contract.pixel_composition.is_some() {
    artifacts.push(PreviewArtifact {
        id: format!("{}-pixel-composition", frame.id),
        title: format!("{} Pixel Composition", frame.title),
        artifact_type: ArtifactType::PixelComposition,
        path: pixel_composition_path(frame),
        width: None,
        height: None,
    });
}
```

5. Add the path helper:

```rust
fn pixel_composition_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.pixel-composition.json", frame.id))
}
```

6. Update any `PreviewScenarioFiles` test literals in `src/dev_preview/export.rs` to include `pixel_composition: None`.

- [ ] **Step 5: Emit schema 2 Pixel art and remove terminal reference rows**

In `src/dev_preview/pixel.rs`:

1. Add imports:

```rust
use crate::dev_preview::export::{
    PreviewPixelArtCellArtifact, PreviewPixelCompositionArtifact,
    PreviewPixelCompositionComparisonArtifact, PreviewPixelCompositionContextArtifact,
    PreviewPixelProtectedRegionArtifact, PIXEL_COMPOSITION_SCHEMA_VERSION,
};
```

Merge them with the existing `use crate::dev_preview::export::{ ... }` block.

2. In `render_pixel_bundle`, remove these lines:

```rust
summary_lines.push("terminal reference".to_string());
summary_lines.extend(render_terminal_reference_lines(&request));
```

3. Delete `render_terminal_reference_lines`.

4. Replace `pixel_art_sidecar` with:

```rust
fn pixel_art_sidecar(
    input: &PixelPetInput,
    reference: &PixelPetArtReference,
) -> PreviewPixelArtArtifact {
    let protected_bounds = reference
        .protected_regions
        .iter()
        .map(protected_region_artifact)
        .collect::<Vec<_>>();
    let signature_regions = protected_bounds
        .iter()
        .filter(|region| region.role == "signature")
        .cloned()
        .collect();

    PreviewPixelArtArtifact {
        schema_version: PIXEL_ART_SCHEMA_VERSION,
        species: input.identity.species.as_str().to_string(),
        stage: input.identity.stage.as_str().to_string(),
        mood: input.mood.as_str().to_string(),
        reference_checksum: format!("{:016x}", reference.reference_checksum.0),
        width_cells: reference.width_cells,
        height_cells: reference.height_cells,
        body_bounds: reference.body_bounds,
        foot_contact: reference.foot_contact.clone(),
        role_counts: reference.role_counts.clone(),
        role_cells: reference
            .occupied_cells
            .iter()
            .map(|cell| PreviewPixelArtCellArtifact {
                x: cell.x,
                y: cell.y,
                role: cell.role.as_str(),
            })
            .collect(),
        protected_bounds,
        signature_regions,
        cue_coverage: reference.cue_coverage.clone(),
    }
}

fn protected_region_artifact(
    region: &crate::presentation::pixel::PixelProtectedRegion,
) -> PreviewPixelProtectedRegionArtifact {
    PreviewPixelProtectedRegionArtifact {
        id: region.id,
        role: region.role,
        bounds: region.bounds,
        cell_count: region.cell_count,
    }
}
```

- [ ] **Step 6: Add composition sidecar helper**

In `src/dev_preview/pixel.rs`, add:

```rust
fn pixel_composition_sidecar(
    frame_id: &str,
    reference: &PixelPetArtReference,
    tank_life_available: bool,
) -> PreviewPixelCompositionArtifact {
    let deferred_contexts = if tank_life_available {
        Vec::new()
    } else {
        vec!["tank-life-unavailable-for-pixel-runtime".to_string()]
    };
    let protected_regions = reference
        .protected_regions
        .iter()
        .map(protected_region_artifact)
        .collect::<Vec<_>>();

    PreviewPixelCompositionArtifact {
        schema_version: PIXEL_COMPOSITION_SCHEMA_VERSION,
        frame_id: frame_id.to_string(),
        context: PreviewPixelCompositionContextArtifact {
            surface: "companion-round-preview".to_string(),
            props_available: false,
            tank_life_available,
            evidence_mode: "read-only-comparison".to_string(),
        },
        comparison: PreviewPixelCompositionComparisonArtifact {
            protected_region_count: protected_regions.len(),
            prop_cells_near_protected_regions: 0,
            tank_life_cells_near_protected_regions: 0,
            occlusion_conflicts: Vec::new(),
            deferred_contexts,
        },
        protected_regions,
    }
}
```

This is deliberately conservative: it records the current Pixel runtime context without adding prop or tank-life placement behavior.

- [ ] **Step 7: Run Preview schema/privacy tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_pixel_writes_art_and_fit_sidecars dev_preview_pixel_privacy_outputs_omit_terminal_reference_rows dev_preview_pixel_composition_artifact_has_own_manifest_slot -- --nocapture
```

Expected: PASS after the `pixel-tank-composition` fixture is added in Task 4. If this is run before Task 4, only the composition fixture assertion may still fail; run it again after Task 4 before committing Task 4.

- [ ] **Step 8: Commit Task 3**

Commit Task 3 only after schema `2` and privacy tests pass, or after Task 4 if the composition fixture is required for the last test:

```bash
git add src/dev_preview/export.rs src/dev_preview/contract.rs src/dev_preview/scenarios.rs src/dev_preview/pixel.rs tests/dev_preview.rs
git commit -m "feat(preview): add pixel identity artifact contracts"
```

---

### Task 4: Add Cast Identity Fixtures, Matrix Grouping, And Composition Evidence

**Files:**
- Modify: `src/dev_preview/pixel.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Test: `tests/dev_preview.rs`

**Interfaces:**
- Produces Preview scenario IDs:
  - `pixel-fuzz-s3-locket`
  - `pixel-blob-s3-body`
  - `pixel-ghost-s3-wisp`
  - `pixel-glitch-s4-repair`
  - `pixel-crystal-s5-facets`
  - `pixel-mech-s5-hardbody`
  - `pixel-cast-identity-matrix`
  - `pixel-tank-composition`
- Preserves existing Preview scenario IDs:
  - `pixel-fuzz-s3-content-idle`
  - `pixel-glitch-s4-feed-pulse`
  - `pixel-species-matrix`

- [ ] **Step 1: Write failing fixture and matrix tests**

Append these tests to `tests/dev_preview.rs`:

```rust
const PIXEL_CAST_IDS: [&str; 6] = [
    "pixel-fuzz-s3-locket",
    "pixel-blob-s3-body",
    "pixel-ghost-s3-wisp",
    "pixel-glitch-s4-repair",
    "pixel-crystal-s5-facets",
    "pixel-mech-s5-hardbody",
];

#[test]
fn dev_preview_pixel_cast_identity_writes_six_real_frame_artifacts() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    for id in PIXEL_CAST_IDS {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "pixel");
        assert_eq!(scenario["files"]["pixel"], format!("frames/{id}.pixel.json"));
        assert_eq!(scenario["files"]["pixel_art"], format!("frames/{id}.pixel-art.json"));
        assert_eq!(scenario["files"]["pixel_fit"], format!("frames/{id}.pixel-fit.json"));
        assert!(run.out.join(format!("frames/{id}.pixel.json")).is_file());
        assert!(run.out.join(format!("frames/{id}.pixel-art.json")).is_file());
        assert!(run.out.join(format!("frames/{id}.pixel-fit.json")).is_file());

        let art = run.read_json(&format!("frames/{id}.pixel-art.json"));
        assert_eq!(art["schema_version"], 2);
        assert!(art["role_cells"].as_array().unwrap().len() > 20);
        assert!(art["protected_bounds"].as_array().unwrap().iter().any(|region| {
            region["id"] == "face"
        }));
    }
}

#[test]
fn dev_preview_pixel_cast_matrix_references_real_cast_frames() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    let manifest = run.manifest();
    let matrix = scenario(&manifest, "pixel-cast-identity-matrix");
    let referenced = matrix["inputs"]["cast_frame_ids"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(referenced, PIXEL_CAST_IDS);
    assert!(matrix["files"].get("pixel").is_none());

    let html = std::fs::read_to_string(run.out.join("index.html")).unwrap();
    for id in PIXEL_CAST_IDS {
        assert!(
            html.contains(&format!("data-pixel-frame=\"frames/{id}.pixel.json\"")),
            "matrix review must expose canvas for {id}"
        );
    }
}

#[test]
fn dev_preview_pixel_hero_cues_have_expected_coverage() {
    let run = PreviewRun::new();

    run.run_success("pixel");

    for (id, cue) in [
        ("pixel-fuzz-s3-locket", "locket"),
        ("pixel-glitch-s4-repair", "repair_mark"),
        ("pixel-crystal-s5-facets", "facet"),
    ] {
        let art = run.read_json(&format!("frames/{id}.pixel-art.json"));
        let coverage = &art["cue_coverage"][cue];
        assert!(coverage["expected"].as_u64().unwrap() > 0, "{id} missing expected {cue}");
        assert_eq!(coverage["expected"], coverage["present"], "{id} did not promote {cue}");
    }
}
```

- [ ] **Step 2: Run the new fixture tests to verify failures**

Run:

```bash
cargo test --features dev-preview --test dev_preview dev_preview_pixel_cast_identity_writes_six_real_frame_artifacts dev_preview_pixel_cast_matrix_references_real_cast_frames dev_preview_pixel_hero_cues_have_expected_coverage -- --nocapture
```

Expected: FAIL because the new cast fixtures and matrix grouping do not exist.

- [ ] **Step 3: Change Pixel preview summaries to owned strings**

In `src/dev_preview/pixel.rs`, change `render_pixel_bundle` to take owned summary lines:

```rust
fn render_pixel_bundle(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    lines: Vec<String>,
    intent: &'static str,
) -> PreviewPixelBundle {
    let (artifacts, input, request) = render_pixel_artifact(ctx, fixture, fixture.elapsed_ms);
    let vm = fixture_view_model(fixture, ctx.fixed_now);
    let dimensions = PreviewDimensions {
        width: artifacts.frame.width,
        height: artifacts.frame.height,
    };
    let mut summary_lines = lines;
    summary_lines.extend(artifacts.fit_status_lines.clone());
    let mut frame = summary_frame(fixture.id, fixture.title, &summary_lines);
    frame.contract.pixel = Some(artifacts.frame);
    frame.contract.pixel_art = Some(artifacts.art);
    frame.contract.pixel_fit = Some(artifacts.fit);

    PreviewScenarioBundle::from_parts_with_dimensions(
        frame,
        PreviewScenarioKind::Pixel,
        intent,
        dimensions,
        scenario_inputs(&input, &vm, fixture.elapsed_ms),
        None,
        Vec::new(),
    )
}
```

Update existing call sites by replacing arrays such as:

```rust
&["species fuzz", "stage s3 pup", "mood content", "pose idle"]
```

with:

```rust
vec![
    "species fuzz".to_string(),
    "stage s3 pup".to_string(),
    "mood content".to_string(),
    "pose idle".to_string(),
]
```

- [ ] **Step 4: Add cast fixtures**

In `src/dev_preview/pixel.rs`, change `pixel_bundles` so it keeps the existing three readiness bundles and appends these bundles before `pixel-tank-composition`:

```rust
let cast_fixtures = [
    PixelFixture {
        id: "pixel-fuzz-s3-locket",
        title: "Pixel Fuzz S3 Locket",
        species: Species::Fuzz,
        stage: Stage::S3,
        mood: Mood::Content,
        asleep: false,
        calm: false,
        burst_level: 0.0,
        pulse_age_ms: None,
        elapsed_ms: 480,
    },
    PixelFixture {
        id: "pixel-blob-s3-body",
        title: "Pixel Blob S3 Body",
        species: Species::Blob,
        stage: Stage::S3,
        mood: Mood::Content,
        asleep: false,
        calm: false,
        burst_level: 0.25,
        pulse_age_ms: None,
        elapsed_ms: 520,
    },
    PixelFixture {
        id: "pixel-ghost-s3-wisp",
        title: "Pixel Ghost S3 Wisp",
        species: Species::Ghost,
        stage: Stage::S3,
        mood: Mood::Content,
        asleep: false,
        calm: true,
        burst_level: 0.15,
        pulse_age_ms: None,
        elapsed_ms: 560,
    },
    PixelFixture {
        id: "pixel-glitch-s4-repair",
        title: "Pixel Glitch S4 Repair",
        species: Species::Glitch,
        stage: Stage::S4,
        mood: Mood::Content,
        asleep: false,
        calm: false,
        burst_level: 0.9,
        pulse_age_ms: Some(300),
        elapsed_ms: 300,
    },
    PixelFixture {
        id: "pixel-crystal-s5-facets",
        title: "Pixel Crystal S5 Facets",
        species: Species::Crystal,
        stage: Stage::S5,
        mood: Mood::Happy,
        asleep: false,
        calm: false,
        burst_level: 0.35,
        pulse_age_ms: None,
        elapsed_ms: 720,
    },
    PixelFixture {
        id: "pixel-mech-s5-hardbody",
        title: "Pixel Mech S5 Hardbody",
        species: Species::Mech,
        stage: Stage::S5,
        mood: Mood::Content,
        asleep: false,
        calm: false,
        burst_level: 0.45,
        pulse_age_ms: None,
        elapsed_ms: 640,
    },
];
```

Map each fixture through `render_pixel_bundle` with review prompts:

```rust
for fixture in cast_fixtures {
    bundles.push(render_pixel_bundle(
        ctx,
        fixture,
        vec![
            format!("species {}", fixture.species.as_str()),
            format!("stage {}", fixture.stage.as_str()),
            "cast identity review".to_string(),
        ],
        "Review a rendered Pixel cast identity frame with promoted cue roles.",
    ));
}
```

- [ ] **Step 5: Add matrix grouping frame**

In `src/dev_preview/pixel.rs`, add:

```rust
const PIXEL_CAST_IDS: [&str; 6] = [
    "pixel-fuzz-s3-locket",
    "pixel-blob-s3-body",
    "pixel-ghost-s3-wisp",
    "pixel-glitch-s4-repair",
    "pixel-crystal-s5-facets",
    "pixel-mech-s5-hardbody",
];

fn pixel_cast_identity_matrix_bundle() -> PreviewPixelBundle {
    let lines = vec![
        "pixel cast identity matrix".to_string(),
        "fuzz blob ghost".to_string(),
        "glitch crystal mech".to_string(),
        "see linked pixel frame canvases".to_string(),
    ];
    let mut frame = summary_frame("pixel-cast-identity-matrix", "Pixel Cast Identity Matrix", &lines);
    frame.extra_inputs.insert(
        "cast_frame_ids".to_string(),
        json!(PIXEL_CAST_IDS),
    );
    PreviewScenarioBundle::from_parts_with_dimensions(
        frame,
        PreviewScenarioKind::Pixel,
        "Review the six real Pixel cast frames together; this grouping is not a stand-in for the frame artifacts.",
        PreviewDimensions { width: 36, height: 4 },
        BTreeMap::from([("cast_frame_ids".to_string(), json!(PIXEL_CAST_IDS))]),
        None,
        vec![
            "Confirm the six linked Pixel canvases read as distinct species.".to_string(),
            "Confirm the grouping does not replace the individual frame artifacts.".to_string(),
        ],
    )
}
```

Push this bundle after the six cast fixtures.

- [ ] **Step 6: Add composition fixture**

In `src/dev_preview/pixel.rs`, add a composition fixture after the matrix:

```rust
fn pixel_tank_composition_bundle(ctx: &PreviewRenderContext) -> PreviewPixelBundle {
    let fixture = PixelFixture {
        id: "pixel-tank-composition",
        title: "Pixel Tank Composition",
        species: Species::Fuzz,
        stage: Stage::S3,
        mood: Mood::Content,
        asleep: false,
        calm: false,
        burst_level: 0.2,
        pulse_age_ms: None,
        elapsed_ms: 480,
    };
    let (artifacts, input, _request, reference) =
        render_pixel_artifact_with_reference(ctx, fixture, fixture.elapsed_ms);
    let vm = fixture_view_model(fixture, ctx.fixed_now);
    let mut frame = summary_frame(
        fixture.id,
        fixture.title,
        &[
            "pixel tank composition".to_string(),
            "existing context evidence".to_string(),
            "no runtime placement mutation".to_string(),
        ],
    );
    frame.contract.pixel = Some(artifacts.frame);
    frame.contract.pixel_art = Some(artifacts.art);
    frame.contract.pixel_fit = Some(artifacts.fit);
    frame.contract.pixel_composition =
        Some(pixel_composition_sidecar(fixture.id, &reference, false));

    PreviewScenarioBundle::from_parts_with_dimensions(
        frame,
        PreviewScenarioKind::Pixel,
        "Record Pixel protected regions against current companion context without adding live prop or tank-life behavior.",
        PreviewDimensions { width: 96, height: 96 },
        scenario_inputs(&input, &vm, fixture.elapsed_ms),
        None,
        vec![
            "Confirm protected face and signature regions are present.".to_string(),
            "Confirm unavailable tank-life context is recorded as deferred rather than implemented here.".to_string(),
        ],
    )
}
```

Add this helper near `render_pixel_artifact` so the composition fixture can access the sanitized reference without changing existing callers:

```rust
fn render_pixel_artifact_with_reference(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    elapsed_ms: u16,
) -> (
    PixelPreviewArtifacts,
    PixelPetInput,
    crate::presentation::pixel::PixelArtReferenceRequest,
    PixelPetArtReference,
) {
    let base = ctx.fixed_now;
    let now = base + time::Duration::milliseconds(i64::from(elapsed_ms));
    let pulse_anchor = now;
    let vm = fixture_view_model(fixture, pulse_anchor);
    let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(&vm, now);
    let mut reference_provider = PixelArtReferenceProvider::default();
    let art_reference = reference_provider.reference_for(&request);
    let mut state = PixelRendererState::new(&input, base);
    let frame = render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &art_reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    });
    (
        PixelPreviewArtifacts {
            frame: pixel_artifact(&frame, &input, elapsed_ms),
            art: pixel_art_sidecar(&input, &art_reference),
            fit: pixel_fit_sidecar(&frame, &vm),
            fit_status_lines: render_fit_status_lines(&frame, &vm),
        },
        input,
        request,
        art_reference,
    )
}

```

Leave `render_pixel_artifact`, `render_pixel_artifact_with_pulse_anchor`, and `render_pixel_artifact_with_provider` in place for existing readiness fixtures and animation strips.

- [ ] **Step 7: Update exact manifest ID tests**

Update the exact `ids` expectations in `tests/dev_preview.rs` and `src/dev_preview/scenarios.rs` so the existing pixel IDs are followed by:

```rust
"pixel-fuzz-s3-locket".to_string(),
"pixel-blob-s3-body".to_string(),
"pixel-ghost-s3-wisp".to_string(),
"pixel-glitch-s4-repair".to_string(),
"pixel-crystal-s5-facets".to_string(),
"pixel-mech-s5-hardbody".to_string(),
"pixel-cast-identity-matrix".to_string(),
"pixel-tank-composition".to_string(),
```

Also extend the expected file list in `dev_preview_all_includes_expected_artifacts` with `.pixel.json`, `.pixel-art.json`, `.pixel-fit.json` for the six cast frames and `.pixel-composition.json` for `pixel-tank-composition`.

- [ ] **Step 8: Run Preview tests**

Run:

```bash
cargo test --features dev-preview --test dev_preview -- --nocapture
```

Expected: PASS.

- [ ] **Step 9: Commit Task 4**

```bash
git add src/dev_preview/pixel.rs src/dev_preview/scenarios.rs tests/dev_preview.rs
git commit -m "feat(preview): add pixel cast identity fixtures"
```

---

### Task 5: Full Verification And Review Evidence

**Files:**
- Create: `docs/superpowers/measurements/2026-07-08-glorp-pixel-cast-identity-tank-composition-review.md`
- No production code changes.

**Interfaces:**
- Consumes: all artifacts from Tasks 1 through 4.
- Produces: human-readable evidence path for manual cast identity review.

- [ ] **Step 1: Run focused test suite**

Run:

```bash
cargo test --test pixel_art_reference
cargo test --test pixel_renderer
cargo test --features dev-preview --test dev_preview
cargo test --test pixel_fit
```

Expected: each command exits `0` with no failed tests.

- [ ] **Step 2: Run full test suite**

Run:

```bash
cargo test
```

Expected: exits `0` with no failed tests.

- [ ] **Step 3: Generate Preview Lab bundle**

Run:

```bash
cargo run -- dev-preview --scenario pixel --out target/glorp-preview-pixel-cast-identity
```

Expected output includes `target/glorp-preview-pixel-cast-identity`, and these files exist:

```text
target/glorp-preview-pixel-cast-identity/manifest.json
target/glorp-preview-pixel-cast-identity/index.html
target/glorp-preview-pixel-cast-identity/frames/pixel-fuzz-s3-locket.pixel.json
target/glorp-preview-pixel-cast-identity/frames/pixel-blob-s3-body.pixel.json
target/glorp-preview-pixel-cast-identity/frames/pixel-ghost-s3-wisp.pixel.json
target/glorp-preview-pixel-cast-identity/frames/pixel-glitch-s4-repair.pixel.json
target/glorp-preview-pixel-cast-identity/frames/pixel-crystal-s5-facets.pixel.json
target/glorp-preview-pixel-cast-identity/frames/pixel-mech-s5-hardbody.pixel.json
target/glorp-preview-pixel-cast-identity/frames/pixel-tank-composition.pixel-composition.json
```

- [ ] **Step 4: Inspect privacy contract with shell checks**

Run:

```bash
rg -n "terminal reference|fixture-seed|very-secret-seed|/Users/|prompt|response|transcript|diagnostic|source_breakdown" target/glorp-preview-pixel-cast-identity
```

Expected: no matches.

Run:

```bash
jq -r '.scenarios[] | select(.id=="pixel-cast-identity-matrix") | .inputs.cast_frame_ids[]' target/glorp-preview-pixel-cast-identity/manifest.json
```

Expected:

```text
pixel-fuzz-s3-locket
pixel-blob-s3-body
pixel-ghost-s3-wisp
pixel-glitch-s4-repair
pixel-crystal-s5-facets
pixel-mech-s5-hardbody
```

- [ ] **Step 5: Write measurement review note**

Create `docs/superpowers/measurements/2026-07-08-glorp-pixel-cast-identity-tank-composition-review.md`:

```markdown
# Glorp Pixel Cast Identity And Tank Composition Review

- Date: 2026-07-08
- Spec: `docs/superpowers/specs/2026-07-08-glorp-pixel-cast-identity-tank-composition-design.md`
- Preview bundle: `target/glorp-preview-pixel-cast-identity`

## Automated Evidence

| Gate | Evidence | Status | Notes |
| --- | --- | --- | --- |
| Role promotion | `cargo test --test pixel_art_reference` | pass | Locket, facet, repair mark, outline, appendage, foot-contact, protected-region, and cue-coverage tests passed. |
| Renderer impact | `cargo test --test pixel_renderer` | pass | Promoted roles change visible pixels and all species/stages still render non-empty frames. |
| Preview contract | `cargo test --features dev-preview --test dev_preview` | pass | Pixel art schema `2`, composition sidecar, six cast fixtures, matrix grouping, and privacy tests passed. |
| Fit/HUD | `cargo test --test pixel_fit` | pass | Existing HUD-safe fit tests passed. |
| Full suite | `cargo test` | pass | Full suite passed after the focused checks. |
| Privacy grep | `rg -n "terminal reference|fixture-seed|very-secret-seed|/Users/|prompt|response|transcript|diagnostic|source_breakdown" target/glorp-preview-pixel-cast-identity` | pass | No matches. |

## Review Artifacts

- `target/glorp-preview-pixel-cast-identity/index.html`
- `target/glorp-preview-pixel-cast-identity/manifest.json`
- `target/glorp-preview-pixel-cast-identity/frames/pixel-fuzz-s3-locket.pixel-art.json`
- `target/glorp-preview-pixel-cast-identity/frames/pixel-glitch-s4-repair.pixel-art.json`
- `target/glorp-preview-pixel-cast-identity/frames/pixel-crystal-s5-facets.pixel-art.json`
- `target/glorp-preview-pixel-cast-identity/frames/pixel-mech-s5-hardbody.pixel-art.json`
- `target/glorp-preview-pixel-cast-identity/frames/pixel-tank-composition.pixel-composition.json`

## Manual Review

| Gate | Status | Reviewer Notes |
| --- | --- | --- |
| Six-species cast identity matrix | pending | Needs visual approval from Drew or reviewer. |
| Hero cue legibility | pending | Needs visual approval for Fuzz locket, Glitch repair marks, Crystal facets, and Mech hardbody. |
| Tank composition evidence | pending | Needs visual approval that protected regions remain readable in the existing companion context. |

## Rollout Status

Pixel remains opt-in. This review does not recommend a default flip.
```

If any command in Steps 1 through 4 fails, write the actual failing command and status in the table instead of `pass`, keep the manual review gates `pending`, and do not claim default readiness.

- [ ] **Step 6: Run doc checks**

Run:

```bash
git diff --check
rg -n "T[B]D|TO[D]O|FI[X]ME|\\?\\?" docs/superpowers/measurements/2026-07-08-glorp-pixel-cast-identity-tank-composition-review.md
```

Expected: `git diff --check` exits `0`; `rg` returns no matches.

- [ ] **Step 7: Commit Task 5**

```bash
git add docs/superpowers/measurements/2026-07-08-glorp-pixel-cast-identity-tank-composition-review.md
git commit -m "docs: record pixel cast identity review"
```

---

## Final Verification Before Merge Or Handoff

Run the full verification set after Task 5:

```bash
cargo test --test pixel_art_reference
cargo test --test pixel_renderer
cargo test --features dev-preview --test dev_preview
cargo test --features dev-preview dev_preview::scenarios
cargo test --features dev-preview dev_preview::export
cargo test --test pixel_fit
cargo test
cargo run -- dev-preview --scenario pixel --out target/glorp-preview-pixel-cast-identity
git diff --check
```

Expected:

- Every test command exits `0`.
- Preview generation exits `0`.
- `target/glorp-preview-pixel-cast-identity/index.html` exists.
- `target/glorp-preview-pixel-cast-identity/manifest.json` lists all six cast fixture IDs.
- `target/glorp-preview-pixel-cast-identity/frames/pixel-tank-composition.pixel-composition.json` exists.
- `git diff --check` exits `0`.

After verification, request review before merging or default-readiness claims. Pixel remains opt-in until a separate default-flip decision is approved.
