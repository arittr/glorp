# Glorp Species Room Dialects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Glitch and Crystal rooms visibly distinct under identical earned-room inputs while preserving earned-prop identity and adding deterministic Preview Lab coverage for all six species dialects.

**Architecture:** Add species dialect as semantic room profile state, then let room/ambient/prop renderers express the existing earned-prop room through that dialect. Preview Lab becomes the review contract: strict Glitch/Crystal truecolor and flat pairs, a six-species matrix, masked room artifacts, and symbol-based tests that ignore pet art and shared prop targets.

**Tech Stack:** Rust, Ratatui buffers/layouts, serde manifest exports, existing `dev-preview` hidden command, Cargo integration tests, Preview Lab artifacts.

---

## File Structure

| File | Responsibility |
|---|---|
| `src/tui/room.rs` | Define `RoomDialectKey`, `RoomDialectStatus`, `RoomSpeciesDialect`; derive dialect in `RoomLifeProfile`; apply dialect to room glyph symbols/zone bias. |
| `src/tui/panels/pet.rs` | Strengthen species ambient glyph/floor behavior for Glitch and Crystal without changing pet/prop readability. |
| `src/tui/component/habitat_props.rs` | Add species-aware shape variants in the correct Trophy and Accent render paths while preserving prop identity, colors, ids, and target ids. |
| `src/dev_preview/export.rs` | Add optional `room_masked_text` manifest file field and masked room text writer. |
| `src/dev_preview/scenarios.rs` | Write masked room artifacts for strict dialect fixtures, emit manifest metadata, add artifact entries, and keep scenario ordering explicit. |
| `src/dev_preview/watch.rs` | Add strict Glitch/Crystal pair fixtures plus the six-species dialect matrix under identical non-species inputs. |
| `tests/dev_preview.rs` | Add Preview Lab contract tests for fixture ids, metadata, masked artifacts, shared inputs, and symbol/zone differences. |
| `src/pet/art.rs` | Tune mature Glitch templates only if full-frame previews still overlap Crystal after room masking passes. |

## Task 1: Preview Contract Tests First

**Files:**
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add species dialect fixture id constants**

Add these constants near the existing watch id constants:

```rust
const SPECIES_DIALECT_STRICT_IDS: [&str; 4] = [
    "watch-species-dialect-glitch",
    "watch-species-dialect-crystal",
    "watch-species-dialect-glitch-flat",
    "watch-species-dialect-crystal-flat",
];

const SPECIES_DIALECT_MATRIX_IDS: [&str; 6] = [
    "watch-species-dialect-fuzz",
    "watch-species-dialect-blob",
    "watch-species-dialect-ghost",
    "watch-species-dialect-glitch",
    "watch-species-dialect-crystal",
    "watch-species-dialect-mech",
];
```

- [ ] **Step 2: Add masked comparison helpers**

Add these helpers near `cells_for_target` and `changed_room_zones`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TestRect {
    x: u64,
    y: u64,
    width: u64,
    height: u64,
}

fn target_rect(layout: &Value, target: &str) -> TestRect {
    let rect = &layout["targets"][target];
    TestRect {
        x: rect["x"].as_u64().unwrap(),
        y: rect["y"].as_u64().unwrap(),
        width: rect["width"].as_u64().unwrap(),
        height: rect["height"].as_u64().unwrap(),
    }
}

fn rect_contains(rect: TestRect, cell: &Value) -> bool {
    let x = cell["x"].as_u64().unwrap();
    let y = cell["y"].as_u64().unwrap();
    x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
}

fn prop_target_ids(layout: &Value) -> Vec<String> {
    layout["targets"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|id| id.starts_with("watch.prop."))
        .cloned()
        .collect()
}

fn union_prop_mask_rects(left: &Value, right: &Value) -> Vec<TestRect> {
    let mut ids = prop_target_ids(left);
    ids.extend(prop_target_ids(right));
    ids.sort();
    ids.dedup();

    let mut rects = Vec::new();
    for id in ids {
        if left["targets"].get(&id).is_some() {
            rects.push(target_rect(left, &id));
        }
        if right["targets"].get(&id).is_some() {
            rects.push(target_rect(right, &id));
        }
    }
    rects
}

fn masked_room_cells(cells: &Value, layout: &Value, pair_layout: &Value) -> Vec<Value> {
    let mut masks = vec![target_rect(layout, "watch.pet.art")];
    if layout["targets"].get("watch.pet.speech").is_some() {
        masks.push(target_rect(layout, "watch.pet.speech"));
    }
    masks.extend(union_prop_mask_rects(layout, pair_layout));

    cells_for_target(cells, layout, "watch.room.effect")
        .into_iter()
        .map(|mut cell| {
            if masks.iter().any(|mask| rect_contains(*mask, &cell)) {
                cell["symbol"] = Value::String(" ".to_string());
                cell["fg"] = Value::Null;
            }
            cell
        })
        .collect()
}
```

- [ ] **Step 3: Add failing fixture/artifact tests**

Add:

```rust
#[test]
fn dev_preview_species_dialect_fixtures_have_manifest_contract() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    for id in SPECIES_DIALECT_STRICT_IDS {
        let scenario = scenario(&manifest, id);
        assert_eq!(scenario["kind"], "watch");
        assert_eq!(scenario["inputs"]["comparison_group"], "species-dialect-glitch-crystal");
        assert!(scenario["inputs"]["room_dialect"].is_string(), "{id} missing room_dialect");
        assert!(scenario["inputs"]["dialect_status"].is_string(), "{id} missing dialect_status");
        assert!(scenario["inputs"]["shared_input_invariants"].is_object(), "{id} missing shared invariants");
        assert!(scenario["inputs"]["prop_identity_invariants"].is_array(), "{id} missing prop invariants");
        assert_eq!(
            scenario["files"]["room_masked_text"],
            format!("frames/{id}.room-masked.txt")
        );
        assert!(
            run.out.join(format!("frames/{id}.room-masked.txt")).is_file(),
            "missing masked room artifact for {id}"
        );
    }
}

#[test]
fn dev_preview_species_dialect_matrix_lists_all_species() {
    let run = PreviewRun::new();

    run.run_success("watch");

    let manifest = run.manifest();
    let species = SPECIES_DIALECT_MATRIX_IDS
        .iter()
        .map(|id| scenario(&manifest, id)["inputs"]["species"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        species,
        vec!["fuzz", "blob", "ghost", "glitch", "crystal", "mech"]
    );

    for id in SPECIES_DIALECT_MATRIX_IDS {
        let scenario = scenario(&manifest, id);
        assert!(scenario["inputs"]["room_dialect"].is_string(), "{id} missing dialect");
        assert!(scenario["inputs"]["dialect_status"].is_string(), "{id} missing status");
    }
}
```

- [ ] **Step 4: Add failing masked symbol comparison tests**

Add:

```rust
#[test]
fn dev_preview_glitch_and_crystal_dialects_differ_after_masking() {
    let run = PreviewRun::new();

    run.run_success("watch");

    assert_species_dialect_pair_differs(&run, "watch-species-dialect-glitch", "watch-species-dialect-crystal");
    assert_species_dialect_pair_differs(
        &run,
        "watch-species-dialect-glitch-flat",
        "watch-species-dialect-crystal-flat",
    );
}

fn assert_species_dialect_pair_differs(run: &PreviewRun, left_id: &str, right_id: &str) {
    let left_cells = read_cells(run, left_id);
    let left_layout = read_layout(run, left_id);
    let right_cells = read_cells(run, right_id);
    let right_layout = read_layout(run, right_id);

    let left_room = masked_room_cells(&left_cells, &left_layout, &right_layout);
    let right_room = masked_room_cells(&right_cells, &right_layout, &left_layout);
    let changed = changed_cells_by_symbol(&left_room, &right_room);
    let rect = target_rect(&left_layout, "watch.room.effect");
    let zones = changed_room_zones(&left_room, &right_room, rect.width, rect.height);

    assert!(
        changed >= 12,
        "{left_id} and {right_id} should differ by at least 12 masked room symbols; changed {changed}"
    );
    assert!(
        zones.len() >= 2,
        "{left_id} and {right_id} should differ across at least two zones; got {zones:?}"
    );
    assert!(
        zones.contains("floor") || zones.contains("left-anchor") || zones.contains("right-anchor"),
        "expected floor or anchor-zone dialect difference; got {zones:?}"
    );
    assert!(
        zones.contains("upper-air") || zones.contains("pet-adjacent"),
        "expected upper-air or pet-adjacent dialect difference; got {zones:?}"
    );
}
```

- [ ] **Step 5: Run tests and verify failure**

Run:

```bash
cargo test --test dev_preview species_dialect
```

Expected: FAIL because the new fixture ids and `room_masked_text` manifest field do not exist yet.

## Task 2: Room Dialect Profile and Room Glyph Behavior

**Files:**
- Modify: `src/tui/room.rs`
- Modify: `src/tui/panels/pet.rs`
- Test: `src/tui/room.rs`

- [ ] **Step 1: Add room dialect types and profile field**

In `src/tui/room.rs`, import `Species` and add the dialect types near `RoomLifeProfile`:

```rust
use crate::pet::generation::Species;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomDialectKey {
    Fuzz,
    Blob,
    Ghost,
    Glitch,
    Crystal,
    Mech,
}

impl RoomDialectKey {
    pub const fn as_str(self) -> &'static str {
        match self {
            RoomDialectKey::Fuzz => "fuzz",
            RoomDialectKey::Blob => "blob",
            RoomDialectKey::Ghost => "ghost",
            RoomDialectKey::Glitch => "glitch",
            RoomDialectKey::Crystal => "crystal",
            RoomDialectKey::Mech => "mech",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoomDialectStatus {
    Tuned,
    Default,
}

impl RoomDialectStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            RoomDialectStatus::Tuned => "tuned",
            RoomDialectStatus::Default => "default",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomSpeciesDialect {
    pub species: Species,
    pub key: RoomDialectKey,
    pub status: RoomDialectStatus,
}

impl RoomSpeciesDialect {
    pub const fn for_species(species: Species) -> Self {
        let key = match species {
            Species::Fuzz => RoomDialectKey::Fuzz,
            Species::Blob => RoomDialectKey::Blob,
            Species::Ghost => RoomDialectKey::Ghost,
            Species::Glitch => RoomDialectKey::Glitch,
            Species::Crystal => RoomDialectKey::Crystal,
            Species::Mech => RoomDialectKey::Mech,
        };
        let status = match species {
            Species::Glitch | Species::Crystal => RoomDialectStatus::Tuned,
            Species::Fuzz | Species::Blob | Species::Ghost | Species::Mech => RoomDialectStatus::Default,
        };
        Self { species, key, status }
    }
}
```

Add `pub species_dialect: RoomSpeciesDialect,` to `RoomLifeProfile`.

- [ ] **Step 2: Derive dialect from the view model**

In `derive_room_life_profile`, add:

```rust
let species_dialect = RoomSpeciesDialect::for_species(vm.pet_render.generated_species);
```

and include `species_dialect` in the returned `RoomLifeProfile`.

- [ ] **Step 3: Update room profile test fixtures**

For every `RoomLifeProfile { ... }` literal in `src/tui/room.rs`, add:

```rust
species_dialect: RoomSpeciesDialect::for_species(Species::Crystal),
```

Use `Species::Glitch` only in tests that are explicitly comparing Glitch behavior.

- [ ] **Step 4: Add failing dialect tests**

Add tests in `src/tui/room.rs`:

```rust
#[test]
fn room_profile_derives_species_dialect_from_pet_species() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Glitch;

    let profile = derive_room_life_profile(&vm, datetime!(2026-06-11 10:00 UTC));

    assert_eq!(profile.species_dialect.species, Species::Glitch);
    assert_eq!(profile.species_dialect.key, RoomDialectKey::Glitch);
    assert_eq!(profile.species_dialect.status, RoomDialectStatus::Tuned);
}

#[test]
fn room_glyphs_change_symbols_by_species_dialect_in_flat_mode() {
    let mut glitch = phase_test_profile();
    glitch.species_dialect = RoomSpeciesDialect::for_species(Species::Glitch);
    glitch.biome = RoomBiome {
        primary: RoomBiomeTag::Technical,
        secondary: Some(RoomBiomeTag::Artifact),
    };

    let mut crystal = glitch.clone();
    crystal.species_dialect = RoomSpeciesDialect::for_species(Species::Crystal);

    let area = Rect::new(0, 0, 120, 32);
    let now = datetime!(2026-06-11 10:00 UTC);
    let glitch_glyphs = room_glyphs_for(&glitch, area, &[], now, ColorCapability::Flat, DayPhase::Day);
    let crystal_glyphs = room_glyphs_for(&crystal, area, &[], now, ColorCapability::Flat, DayPhase::Day);
    let changed = glitch_glyphs
        .iter()
        .zip(&crystal_glyphs)
        .filter(|(left, right)| left.glyph != right.glyph)
        .count();
    let changed_zones = glitch_glyphs
        .iter()
        .zip(&crystal_glyphs)
        .filter(|(left, right)| left.glyph != right.glyph)
        .map(|(left, _)| left.zone)
        .collect::<std::collections::BTreeSet<_>>();

    assert!(changed >= 6, "expected at least 6 dialect symbol changes; got {changed}");
    assert!(
        changed_zones.len() >= 2,
        "expected dialect changes in at least two zones; got {changed_zones:?}"
    );
}
```

- [ ] **Step 5: Run tests and verify failure**

Run:

```bash
cargo test --lib species_dialect
```

Expected: the first test passes after profile plumbing; the second fails until room glyph symbols use dialect-specific families.

- [ ] **Step 6: Implement dialect symbol families**

Change `biome_symbols` to accept dialect:

```rust
fn biome_symbols(tag: RoomBiomeTag, dialect: RoomSpeciesDialect) -> &'static [char] {
    match dialect.key {
        RoomDialectKey::Glitch => match tag {
            RoomBiomeTag::Starter => &[':', '.', '_'],
            RoomBiomeTag::Botanical => &[';', '`', ',', '_'],
            RoomBiomeTag::Technical => &[':', ';', '+', '='],
            RoomBiomeTag::Celestial => &['.', ':', '+', '*'],
            RoomBiomeTag::Artifact => &['#', ':', '[', ']'],
            RoomBiomeTag::Cozy => &['_', '-', ':', '.'],
        },
        RoomDialectKey::Crystal => match tag {
            RoomBiomeTag::Starter => &['.', '·', '◇'],
            RoomBiomeTag::Botanical => &['\'', '·', '◇', ','],
            RoomBiomeTag::Technical => &['◇', '+', '·', ':'],
            RoomBiomeTag::Celestial => &['✦', '✧', '·', '*'],
            RoomBiomeTag::Artifact => &['◇', '◆', '·', '°'],
            RoomBiomeTag::Cozy => &['·', '◇', '⌞', '⌟'],
        },
        RoomDialectKey::Fuzz | RoomDialectKey::Blob | RoomDialectKey::Ghost | RoomDialectKey::Mech => match tag {
            RoomBiomeTag::Starter => &['.', '·'],
            RoomBiomeTag::Botanical => &['"', '\'', '`', ','],
            RoomBiomeTag::Technical => &[':', ';', '+', '='],
            RoomBiomeTag::Celestial => &['*', '·', '˚', '.'],
            RoomBiomeTag::Artifact => &['.', 'o', '◇', '°'],
            RoomBiomeTag::Cozy => &['~', '·', '⌞', '⌟'],
        },
    }
}
```

Update `biome_glyphs` to call:

```rust
let style = biome_style(profile.biome.primary, color_capability);
let symbols = biome_symbols(profile.biome.primary, profile.species_dialect);
```

and secondary symbols similarly.

- [ ] **Step 7: Add small dialect zone bias**

Add:

```rust
fn dialect_zone_counts(dialect: RoomSpeciesDialect) -> Vec<(RoomZone, usize)> {
    match dialect.key {
        RoomDialectKey::Glitch => vec![(RoomZone::RightAnchor, 2), (RoomZone::Floor, 1)],
        RoomDialectKey::Crystal => vec![(RoomZone::UpperAir, 2), (RoomZone::Floor, 1)],
        RoomDialectKey::Fuzz | RoomDialectKey::Blob | RoomDialectKey::Ghost | RoomDialectKey::Mech => Vec::new(),
    }
}
```

In `biome_glyphs`, after primary and secondary allocations are placed, place dialect allocations with the same seeded RNG and dialect symbols. Keep the existing `motion_budget` cap unchanged.

- [ ] **Step 8: Run room tests**

Run:

```bash
cargo test --lib room_glyphs
cargo test --lib species_dialect
```

Expected: PASS.

## Task 3: Masked Room Artifact Export

**Files:**
- Modify: `src/dev_preview/export.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Test: `src/dev_preview/export.rs`

- [ ] **Step 1: Extend manifest file schema**

In `PreviewScenarioFiles`, add:

```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub room_masked_text: Option<PathBuf>,
```

Update `sample_manifest()` in `src/dev_preview/export.rs` to set:

```rust
room_masked_text: None,
```

- [ ] **Step 2: Add masked room export types and writer**

In `src/dev_preview/export.rs`, add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreviewMaskRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl PreviewMaskRect {
    fn contains(self, col: u16, row: u16) -> bool {
        col >= self.x
            && col < self.x.saturating_add(self.width)
            && row >= self.y
            && row < self.y.saturating_add(self.height)
    }
}

pub fn write_room_text_frame_masked(
    path: &Path,
    frame: &PreviewFrame,
    target_id: &str,
    masks: &[PreviewMaskRect],
) -> Result<()> {
    let layout = frame
        .layout
        .as_ref()
        .expect("frame should have layout for masked room text");
    let target = layout
        .targets
        .get(target_id)
        .unwrap_or_else(|| panic!("layout should contain {target_id}"));
    let mut text = String::new();
    for row in target.y..target.y + target.height {
        for col in target.x..target.x + target.width {
            if masks.iter().any(|mask| mask.contains(col, row)) {
                text.push(' ');
                continue;
            }
            let cell = frame
                .cells
                .iter()
                .find(|cell| cell.x == col && cell.y == row)
                .expect("frame should contain each coordinate");
            if cell.continuation {
                continue;
            }
            text.push_str(&cell.symbol);
        }
        text.push('\n');
    }
    fs::write(path, text)?;
    Ok(())
}
```

- [ ] **Step 3: Add export unit test**

Add a unit test in `src/dev_preview/export.rs`:

```rust
#[test]
fn masked_room_text_export_replaces_masked_cells_with_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("room-masked.txt");
    let mut frame = sample_frame();
    frame.layout = Some(crate::tui::component::PreviewLayout {
        schema_version: 2,
        frame_id: "frame-one".to_string(),
        mode: "wide".to_string(),
        frame: crate::tui::component::PreviewRect { x: 0, y: 0, width: 2, height: 2 },
        content: crate::tui::component::PreviewRect { x: 0, y: 0, width: 2, height: 2 },
        components: BTreeMap::new(),
        targets: BTreeMap::from([(
            "watch.room.effect".to_string(),
            crate::tui::component::PreviewTarget {
                x: 0,
                y: 0,
                width: 2,
                height: 2,
                owner: "watch.pet".to_string(),
                role: "RoomEffect".to_string(),
                clip: crate::tui::component::PreviewRect { x: 0, y: 0, width: 2, height: 2 },
                z: 5,
                layer: "room-background".to_string(),
                cell_count: None,
            },
        )]),
        decisions: vec![],
    });

    write_room_text_frame_masked(
        &path,
        &frame,
        "watch.room.effect",
        &[PreviewMaskRect { x: 0, y: 0, width: 1, height: 2 }],
    )
    .unwrap();

    assert_eq!(fs::read_to_string(path).unwrap(), " <\n &\"\n");
}
```

- [ ] **Step 4: Wire masked artifact paths in scenarios**

In `src/dev_preview/scenarios.rs`, import the new writer/type and add:

```rust
fn room_masked_text_path(frame: &PreviewFrame) -> PathBuf {
    PathBuf::from(format!("frames/{}.room-masked.txt", frame.id))
}

fn is_strict_species_dialect_frame(id: &str) -> bool {
    matches!(
        id,
        "watch-species-dialect-glitch"
            | "watch-species-dialect-crystal"
            | "watch-species-dialect-glitch-flat"
            | "watch-species-dialect-crystal-flat"
    )
}
```

Set `room_masked_text` in `PreviewScenarioFiles`:

```rust
room_masked_text: if is_strict_species_dialect_frame(&frame.id) {
    Some(room_masked_text_path(frame))
} else {
    None
},
```

- [ ] **Step 5: Compute masks across comparison pairs**

In `src/dev_preview/scenarios.rs`, add helpers that build masks from the union of matching prop targets plus pet/speech:

```rust
fn preview_mask_rect(target: &crate::tui::component::PreviewTarget) -> crate::dev_preview::export::PreviewMaskRect {
    crate::dev_preview::export::PreviewMaskRect {
        x: target.x,
        y: target.y,
        width: target.width,
        height: target.height,
    }
}

fn species_dialect_pair_id(id: &str) -> Option<&'static str> {
    match id {
        "watch-species-dialect-glitch" | "watch-species-dialect-crystal" => Some("truecolor"),
        "watch-species-dialect-glitch-flat" | "watch-species-dialect-crystal-flat" => Some("flat"),
        _ => None,
    }
}
```

Use those helpers before the frame-writing loop:

```rust
let masked_room_masks = masked_room_masks_for_species_dialect_pairs(&frames);
```

and in the frame loop:

```rust
if let Some(masks) = masked_room_masks.get(&frame.id) {
    write_room_text_frame_masked(
        &staging_dir.join(room_masked_text_path(frame)),
        frame,
        "watch.room.effect",
        masks,
    )?;
}
```

Implement `masked_room_masks_for_species_dialect_pairs`:

```rust
fn masked_room_masks_for_species_dialect_pairs(
    frames: &[PreviewFrame],
) -> BTreeMap<String, Vec<PreviewMaskRect>> {
    let mut grouped: BTreeMap<&'static str, Vec<&PreviewFrame>> = BTreeMap::new();
    for frame in frames {
        if let Some(pair_id) = species_dialect_pair_id(&frame.id) {
            grouped.entry(pair_id).or_default().push(frame);
        }
    }

    let mut masks_by_frame = BTreeMap::new();
    for pair_frames in grouped.values() {
        let mut prop_masks = Vec::new();
        for frame in pair_frames {
            let Some(layout) = &frame.layout else {
                continue;
            };
            for (target_id, target) in &layout.targets {
                if target_id.starts_with("watch.prop.") {
                    prop_masks.push(preview_mask_rect(target));
                }
            }
        }
        prop_masks.sort_by_key(|rect| (rect.y, rect.x, rect.width, rect.height));
        prop_masks.dedup();

        for frame in pair_frames {
            let layout = frame
                .layout
                .as_ref()
                .expect("strict species dialect frames should have layout");
            let mut masks = Vec::new();
            if let Some(target) = layout.targets.get("watch.pet.art") {
                masks.push(preview_mask_rect(target));
            }
            if let Some(target) = layout.targets.get("watch.pet.speech") {
                masks.push(preview_mask_rect(target));
            }
            masks.extend(prop_masks.iter().copied());
            masks_by_frame.insert(frame.id.clone(), masks);
        }
    }
    masks_by_frame
}
```

- [ ] **Step 6: Add artifact entry**

In `artifacts_for_frames`, add after the room artifact:

```rust
if is_strict_species_dialect_frame(&frame.id) {
    artifacts.push(PreviewArtifact {
        id: format!("{}-room-masked", frame.id),
        title: format!("{} Masked Room", frame.title),
        artifact_type: ArtifactType::Text,
        path: room_masked_text_path(frame),
        width: Some(frame.width),
        height: Some(frame.height),
    });
}
```

- [ ] **Step 7: Run export/scenario tests**

Run:

```bash
cargo test dev_preview::export
cargo test masked_room_text_export_replaces_masked_cells_with_spaces
cargo test dev_preview::scenarios
```

Expected: export unit test passes; scenario tests still fail until species dialect fixtures exist.

## Task 4: Species Dialect Preview Fixtures and Metadata

**Files:**
- Modify: `src/dev_preview/watch.rs`
- Modify: `src/dev_preview/scenarios.rs`
- Modify: `tests/dev_preview.rs`

- [ ] **Step 1: Add shared species dialect props**

In `src/dev_preview/watch.rs`, add:

```rust
fn species_dialect_props(fixed_now: OffsetDateTime) -> Vec<EarnedHabitatProp> {
    vec![
        EarnedHabitatProp {
            id: HabitatPropId::new("codex_signal_lamp"),
            earned_at: fixed_now - Duration::days(12),
            source: HabitatPropSource::ProviderFirstUse {
                provider_surface: "codex".to_string(),
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_shard_1m"),
            earned_at: fixed_now - Duration::days(11),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 1_000_000.0,
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_orbit_5m"),
            earned_at: fixed_now - Duration::days(9),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 5_000_000.0,
            },
        },
        EarnedHabitatProp {
            id: HabitatPropId::new("token_lantern_10m"),
            earned_at: fixed_now - Duration::days(6),
            source: HabitatPropSource::LifetimeTokens {
                threshold: 10_000_000.0,
            },
        },
    ]
}
```

- [ ] **Step 2: Add species dialect fixtures**

Add:

```rust
fn species_dialect_frame_fixtures(ctx: &PreviewRenderContext) -> Vec<AliveRoomFrameFixture> {
    let fixed_now = ctx.fixed_now;
    let props = species_dialect_props(fixed_now);
    let day_context = DayContext {
        day_phase: DayPhase::Day,
        mature: true,
        ..DayContext::default()
    };
    let mut hot = hot_life_profile(false);
    hot.work_weather = WorkWeather::OutputSparks;

    let mk = |id: &'static str, title: &'static str, species: Species, color_capability: ColorCapability| {
        AliveRoomFrameFixture {
            id,
            title,
            width: 120,
            height: 32,
            species,
            stage: Stage::S6,
            props: props.clone(),
            life: WatchLifeFixture {
                profile: hot.clone(),
                color_capability,
                last_feed_pulse_at: Some(fixed_now - Duration::seconds(5)),
            },
            day_context,
            expected_biome: "Technical",
            expected_emitter: Some("codex_signal_lamp"),
        }
    };

    vec![
        mk("watch-species-dialect-fuzz", "Watch Species Dialect Fuzz", Species::Fuzz, ColorCapability::Truecolor),
        mk("watch-species-dialect-blob", "Watch Species Dialect Blob", Species::Blob, ColorCapability::Truecolor),
        mk("watch-species-dialect-ghost", "Watch Species Dialect Ghost", Species::Ghost, ColorCapability::Truecolor),
        mk("watch-species-dialect-glitch", "Watch Species Dialect Glitch", Species::Glitch, ColorCapability::Truecolor),
        mk("watch-species-dialect-crystal", "Watch Species Dialect Crystal", Species::Crystal, ColorCapability::Truecolor),
        mk("watch-species-dialect-mech", "Watch Species Dialect Mech", Species::Mech, ColorCapability::Truecolor),
        mk("watch-species-dialect-glitch-flat", "Watch Species Dialect Glitch Flat", Species::Glitch, ColorCapability::Flat),
        mk("watch-species-dialect-crystal-flat", "Watch Species Dialect Crystal Flat", Species::Crystal, ColorCapability::Flat),
    ]
}
```

In `watch_frames`, render these fixtures after existing alive-room fixtures and before activity-identity fixtures:

```rust
for fixture in species_dialect_frame_fixtures(ctx) {
    frames.push(render_alive_room_watch_frame(ctx, scratch_dir, fixture)?);
}
```

- [ ] **Step 3: Add dialect metadata to frame extra inputs**

In `render_alive_room_watch_frame`, extend `room_life_profile` JSON:

```rust
"species_dialect": {
    "species": room_profile.species_dialect.species.as_str(),
    "key": room_profile.species_dialect.key.as_str(),
    "status": room_profile.species_dialect.status.as_str()
}
```

- [ ] **Step 4: Add scenario metadata branch**

In `src/dev_preview/scenarios.rs`, add before the `room-` branch:

```rust
id if id.starts_with("watch-species-dialect-") => (
    PreviewScenarioKind::Watch,
    "Review species room dialect rendering under identical earned-room inputs.",
    species_dialect_inputs_for_frame(id, frame),
    vec![
        "Compare paired .room.txt crops first to confirm the same earned room is present.".to_string(),
        "Compare paired .room-masked.txt crops for species dialect differences outside pet art and shared props.".to_string(),
        "Confirm earned props remain recognizable and species changes the room texture, not the prop identity.".to_string(),
    ],
),
```

Add:

```rust
fn species_dialect_inputs_for_frame(id: &str, frame: &PreviewFrame) -> BTreeMap<String, Value> {
    let species = id
        .trim_start_matches("watch-species-dialect-")
        .trim_end_matches("-flat");
    let color_capability = if id.ends_with("-flat") { "flat" } else { "truecolor" };
    let dialect_status = match species {
        "glitch" | "crystal" => "tuned",
        _ => "default",
    };
    BTreeMap::from([
        ("species".to_string(), Value::String(species.to_string())),
        ("room_dialect".to_string(), Value::String(species.to_string())),
        ("dialect_status".to_string(), Value::String(dialect_status.to_string())),
        (
            "comparison_group".to_string(),
            Value::String(if matches!(species, "glitch" | "crystal") {
                "species-dialect-glitch-crystal".to_string()
            } else {
                "species-dialect-matrix".to_string()
            }),
        ),
        ("stage".to_string(), Value::String("s6".to_string())),
        ("day_phase".to_string(), Value::String("day".to_string())),
        ("work_weather".to_string(), Value::String("output-sparks".to_string())),
        ("color_capability".to_string(), Value::String(color_capability.to_string())),
        ("terminal_width".to_string(), json!(frame.width)),
        ("terminal_height".to_string(), json!(frame.height)),
        (
            "earned_prop_ids".to_string(),
            json!(["codex_signal_lamp", "token_shard_1m", "token_orbit_5m", "token_lantern_10m"]),
        ),
        (
            "shared_input_invariants".to_string(),
            json!({
                "stage": "s6",
                "day_phase": "day",
                "work_weather": "output-sparks",
                "terminal": [120, 32],
                "earned_prop_ids": ["codex_signal_lamp", "token_shard_1m", "token_orbit_5m", "token_lantern_10m"]
            }),
        ),
        (
            "expected_changed_zones".to_string(),
            json!(["floor-or-anchor", "upper-air-or-pet-adjacent"]),
        ),
        (
            "prop_identity_invariants".to_string(),
            json!([
                "prop ids stay stable",
                "target ids stay stable",
                "catalog colors stay stable",
                "base prop object class stays recognizable"
            ]),
        ),
    ])
}
```

- [ ] **Step 5: Update exact scenario id tests**

Add the eight new ids to both exact-order lists in `src/dev_preview/scenarios.rs` tests and `tests/dev_preview.rs`. Place them after `room-dawn-wake-small` and before `watch-activity-identity-ensemble`:

```rust
"watch-species-dialect-fuzz",
"watch-species-dialect-blob",
"watch-species-dialect-ghost",
"watch-species-dialect-glitch",
"watch-species-dialect-crystal",
"watch-species-dialect-mech",
"watch-species-dialect-glitch-flat",
"watch-species-dialect-crystal-flat",
```

- [ ] **Step 6: Run Preview Lab contract tests**

Run:

```bash
cargo test --test dev_preview species_dialect
cargo test dev_preview::scenarios
```

Expected: PASS for metadata/artifact contract tests; masked symbol comparison may still fail until render dialects are visible enough.

## Task 5: Species-Aware Ambient and Prop Shape Variants

**Files:**
- Modify: `src/tui/panels/pet.rs`
- Modify: `src/tui/component/habitat_props.rs`
- Test: `src/tui/panels/pet.rs`
- Test: `src/tui/component/habitat_props.rs`

- [ ] **Step 1: Add ambient dialect tests**

In `src/tui/panels/pet.rs`, add tests near existing ambient palette tests:

```rust
#[test]
fn glitch_and_crystal_ambient_floor_use_distinct_symbol_families() {
    let habitat = Rect::new(0, 0, 80, 20);
    let now = datetime!(2026-06-11 10:00 UTC);
    let glitch = ambient_glyphs_for_phase(
        Species::Glitch,
        Stage::S6,
        habitat,
        &[],
        now,
        ColorCapability::Truecolor,
        DayPhase::Day,
        1.0,
        0,
        Season::Summer,
        None,
    );
    let crystal = ambient_glyphs_for_phase(
        Species::Crystal,
        Stage::S6,
        habitat,
        &[],
        now,
        ColorCapability::Truecolor,
        DayPhase::Day,
        1.0,
        0,
        Season::Summer,
        None,
    );
    let glitch_symbols = glitch.iter().map(|g| g.glyph).collect::<std::collections::HashSet<_>>();
    let crystal_symbols = crystal.iter().map(|g| g.glyph).collect::<std::collections::HashSet<_>>();

    assert!(glitch_symbols.iter().any(|c| ['#', ':', ';', '_', '░', '▒', '▪'].contains(c)));
    assert!(crystal_symbols.iter().any(|c| ['◇', '◆', '✦', '✧', '·'].contains(c)));
    assert_ne!(glitch_symbols, crystal_symbols);
}
```

- [ ] **Step 2: Strengthen ambient palettes**

Update `sky_palette_for`, `floor_palette_for`, and `sky_palette_for_phase` so Glitch uses interface/noise symbols and Crystal uses facet/prism symbols. Keep all glyphs single-column:

```rust
Species::Glitch => &[':', ';', '#', '░', '▒', '▪'],
Species::Crystal => &['✦', '✧', '◇', '◆', '·'],
```

For floor:

```rust
Species::Glitch => &['_', '-', ':', ' ', '░'],
Species::Crystal => &['◇', '·', '.', ' ', ' '],
```

- [ ] **Step 3: Add prop variant tests**

In `src/tui/component/habitat_props.rs`, add tests:

```rust
#[test]
fn selected_accent_glyphs_can_vary_by_species_without_changing_identity() {
    let now = time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();

    assert_ne!(
        accent_glyph(TOKEN_SHARD_1M, Species::Glitch, now),
        accent_glyph(TOKEN_SHARD_1M, Species::Crystal, now)
    );
    assert_eq!(prop_effect_target_path(TOKEN_SHARD_1M).unwrap().as_str(), "watch.prop.token_shard_1m.effect");
    assert_eq!(catalog_prop_by_str(TOKEN_SHARD_1M).unwrap().color, (0x82, 0xcc, 0xd8));
}

#[test]
fn codex_signal_lamp_sprite_can_vary_by_species_without_changing_target_or_color() {
    let now = time::OffsetDateTime::from_unix_timestamp(1_760_000_000).unwrap();

    assert_ne!(
        trophy_sprite(CODEX_SIGNAL_LAMP, Species::Glitch, now),
        trophy_sprite(CODEX_SIGNAL_LAMP, Species::Crystal, now)
    );
    assert_eq!(prop_effect_target_path(CODEX_SIGNAL_LAMP).unwrap().as_str(), "watch.prop.codex_signal_lamp.effect");
    assert_eq!(catalog_prop_by_str(CODEX_SIGNAL_LAMP).unwrap().color, (0xd8, 0x6c, 0x5c));
}
```

- [ ] **Step 4: Thread species through accent path**

Change signatures:

```rust
fn accent_cell_from_anchor(
    id: &str,
    anchor: Position,
    habitat: Rect,
    exclusions: &[Rect],
    pet_layer: HabitatPetLayer,
    species: Species,
    now: time::OffsetDateTime,
) -> Option<HabitatPropCell>
```

and:

```rust
fn accent_glyph(id: &str, species: Species, now: time::OffsetDateTime) -> char
```

Update calls in `stable_accent_cells_by_id` and `render_accent`.

- [ ] **Step 5: Implement selected prop variants**

Use small glyph changes only:

```rust
fn accent_glyph(id: &str, species: Species, now: time::OffsetDateTime) -> char {
    let twinkle = now.unix_timestamp().rem_euclid(12) < 2;
    match (id, species) {
        (TOKEN_SHARD_1M, Species::Glitch) => '#',
        (TOKEN_SHARD_1M, Species::Crystal) => '◆',
        (TOKEN_ORBIT_5M, Species::Glitch) => ']',
        (TOKEN_ORBIT_5M, Species::Crystal) => '°',
        (TOKEN_LANTERN_10M, Species::Glitch) if twinkle => '_',
        (TOKEN_LANTERN_10M, Species::Glitch) => ':',
        (TOKEN_LANTERN_10M, Species::Crystal) if twinkle => '✦',
        (TOKEN_LANTERN_10M, Species::Crystal) => '○',
        (TOKEN_PEBBLE_25K, _) => '▲',
        (TOKEN_SHELL_100K, _) => '◌',
        (TOKEN_SPARK_500K, _) if twinkle => '✦',
        (TOKEN_SPARK_500K, _) => '·',
        (TOKEN_SHARD_1M, _) => '◆',
        (TOKEN_ORBIT_5M, _) => '°',
        (TOKEN_LANTERN_10M, _) if twinkle => '☼',
        (TOKEN_LANTERN_10M, _) => '○',
        _ => '·',
    }
}
```

For `CODEX_SIGNAL_LAMP`, add Glitch and Crystal match arms before the existing lamp arms in `trophy_sprite`; keep footprint height and target id stable:

```rust
CODEX_SIGNAL_LAMP if matches!(species, Species::Glitch) && phase < 4 => &[
    SpriteCell { dx: 0, dy: 0, glyph: '╷' },
    SpriteCell { dx: 0, dy: 1, glyph: '#' },
    SpriteCell { dx: 0, dy: 2, glyph: '_' },
],
CODEX_SIGNAL_LAMP if matches!(species, Species::Glitch) => &[
    SpriteCell { dx: 0, dy: 0, glyph: '_' },
    SpriteCell { dx: 0, dy: 1, glyph: ':' },
    SpriteCell { dx: 0, dy: 2, glyph: '╵' },
],
CODEX_SIGNAL_LAMP if matches!(species, Species::Crystal) && phase < 4 => &[
    SpriteCell { dx: 0, dy: 0, glyph: '╷' },
    SpriteCell { dx: 0, dy: 1, glyph: '◆' },
    SpriteCell { dx: 0, dy: 2, glyph: '╵' },
],
CODEX_SIGNAL_LAMP if matches!(species, Species::Crystal) => &[
    SpriteCell { dx: 0, dy: 0, glyph: '╷' },
    SpriteCell { dx: 0, dy: 1, glyph: '◇' },
    SpriteCell { dx: 0, dy: 2, glyph: '╵' },
],
```

Rename `_species` to `species` when it is used.

- [ ] **Step 6: Run focused render tests**

Run:

```bash
cargo test --lib glitch_and_crystal_ambient_floor_use_distinct_symbol_families
cargo test --lib selected_accent_glyphs_can_vary_by_species_without_changing_identity
cargo test --lib codex_signal_lamp_sprite_can_vary_by_species_without_changing_target_or_color
cargo test --test tui_render
```

Expected: PASS.

## Task 6: Preview Diff Gate and Optional Glitch Pet Art Tuning

**Files:**
- Modify: `tests/dev_preview.rs`
- Modify: `src/pet/art.rs` only if full-frame review still overlaps Crystal

- [ ] **Step 1: Run strict Preview Lab diff gate**

Run:

```bash
cargo test --test dev_preview dev_preview_glitch_and_crystal_dialects_differ_after_masking
```

Expected: PASS. If it fails by too few changed symbols or zones, tune `src/tui/room.rs` dialect symbol families/zone counts before editing pet art.

- [ ] **Step 2: Generate Preview Lab bundle**

Run:

```bash
cargo run -- dev-preview --scenario watch --out target/glorp-preview
```

Expected: command exits 0 and writes `target/glorp-preview/index.html`, the strict dialect `.room.txt` crops, and `.room-masked.txt` crops.

- [ ] **Step 3: Review generated text artifacts**

Inspect these files:

```bash
sed -n '1,80p' target/glorp-preview/frames/watch-species-dialect-glitch.room-masked.txt
sed -n '1,80p' target/glorp-preview/frames/watch-species-dialect-crystal.room-masked.txt
sed -n '1,80p' target/glorp-preview/frames/watch-species-dialect-glitch.txt
sed -n '1,80p' target/glorp-preview/frames/watch-species-dialect-crystal.txt
```

Expected: masked crops show Glitch scanline/interface marks and Crystal facet/prism marks in at least two zones; full frames keep shared earned props recognizable.

- [ ] **Step 4: Tune mature Glitch art only if full frames still read as Crystal**

If the full-frame Glitch pet still reads as a faceted crystal, edit only `GLITCH_ADULT` in `src/pet/art.rs`. Keep the template size and named interpolation slots unchanged. A safe direction is replacing some filled block mass with terminal rows and offsets:

```rust
[
    "  ░#::_ ░  ",
    " ▌▀▀▀ ▀▐  ",
    " ▌ {eyes} ▐# ",
    " :_ {mouth}  ▌ ",
    " ▌ {pattern} ▐ ",
    "  ▀▄{accent}_▌  ",
    "  _ ░_ #   ",
    " :  ░  _#  ",
],
```

Before applying any row, count columns and preserve the existing 11-column template width.

- [ ] **Step 5: Run pet art tests if templates changed**

Run:

```bash
cargo test --lib pet::art
cargo test --test dev_preview dev_preview_pets_writes_species_stage_matrix
```

Expected: PASS. If template width assertions fail, fix the edited rows before continuing.

## Task 7: Final Verification and Review Bundle

**Files:**
- No new code files unless previous tasks required them.

- [ ] **Step 1: Run focused test suite**

Run:

```bash
cargo test --test dev_preview
cargo test dev_preview::scenarios
cargo test dev_preview::habitat_props
cargo test dev_preview::export
cargo test --lib room_glyphs
cargo test --test tui_render
```

Expected: all commands exit 0.

- [ ] **Step 2: Run formatter/checks**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both commands exit 0.

- [ ] **Step 3: Review Preview Lab in browser**

Run:

```bash
cargo run -- dev-preview --scenario watch --out target/glorp-preview
open target/glorp-preview/index.html
```

Expected: preview opens; Glitch/Crystal strict fixtures are visibly different in masked crops and full frames; shared props remain legible.

- [ ] **Step 4: Check git status**

Run:

```bash
git status --short
```

Expected: only intentional source, test, snapshot, spec, and plan files are modified. Generated `target/glorp-preview` artifacts should not appear as tracked changes.
