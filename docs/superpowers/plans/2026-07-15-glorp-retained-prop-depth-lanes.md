# Glorp Retained Prop Depth Lanes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Distribute grounded retained-renderer props across stable rear, middle, and near floor lanes while every prop remains attached to its authored floor, ceiling, wall, or air zone.

**Architecture:** Extend the pure companion composition solver with deterministic floor-lane candidate ordering. Keep authored world Z and every retained GPU contract unchanged; lane placement is a logical-cell composition concern, and the existing frame projection automatically carries the accepted origin, footprint, contact shadow, and tank reservation forward.

**Tech Stack:** Rust, the existing `CompanionComposition` solver, Rust unit tests, retained-scene integration tests, and Preview Lab.

## Global Constraints

- `PropZoneSnapshot` remains the physical attachment authority: floor props contact the substrate, ceiling props remain at the top, wall props remain on their side, and air props remain interior.
- Canonical 18-row floor contacts use exclusive bottom bounds 15, 16, and 17 for rear, middle, and near lanes.
- Lane preference is deterministic from catalog ID and authored depth; it does not depend on `stable_order`, visible inventory length, time, animation phase, or viewport size.
- Existing aperture, HUD, gauge, gutter, collision, active-footprint bottom alignment, shadow, and foreground tank-reservation behavior remains authoritative.
- Do not change `AuthoredDepthSnapshot`, scene-node Z, parent layers, depth-cue scale, parallax, opacity, saturation, renderer ABI, shaders, or non-retained renderers.
- Do not create a compatibility path, physics solver, dynamic rebalancer, or per-frame repacking.
- Implement test-first: observe the new lane behavior fail against current production code before changing production code.

---

### Task 1: Add stable grounded-prop depth lanes

**Files:**
- Modify: `src/presentation/companion_scene/composition.rs:93-188`
- Modify: `src/presentation/companion_scene/composition.rs:264-286`
- Modify: `src/presentation/companion_scene/composition.rs:548-840`
- Verify unchanged behavior: `src/presentation/companion_scene/input.rs:2832-2897`
- Verify integration: `tests/retained_scene.rs`

**Interfaces:**
- Consumes: `PropTopologySnapshot { catalog_id, stable_order, zone, authored_depth, .. }`, `CandidateAnchor`, `candidate_anchors`, and the existing composition exclusions.
- Produces: private `FloorDepthLane`, `floor_lane_order(&PropTopologySnapshot) -> [FloorDepthLane; 3]`, `stable_floor_lane_variant(&str) -> bool`, and `grounded_candidate_anchors(&PropTopologySnapshot, u16, u16, [i16; 2]) -> Vec<CandidateAnchor>`.
- Preserves: `CompanionPropPlacement`, `PropFrameSnapshot`, scene frame slots, GPU buffers, renderer nodes, and every public or serialized contract.

- [ ] **Step 1: Add the failing floor-lane behavior test**

In `composition.rs`'s existing test module, add a small topology builder and a literal fixture. The break this test catches is replacing depth-aware floor candidates with one hardcoded bottom row.

```rust
fn prop_topology(
    catalog_id: &'static str,
    stable_order: u8,
    zone: PropZoneSnapshot,
    authored_depth: AuthoredDepthSnapshot,
) -> PropTopologySnapshot {
    PropTopologySnapshot {
        catalog_id,
        stable_order,
        zone,
        authored_depth,
        presentation_motion: PropPresentationMotion::Static,
    }
}

#[test]
fn grounded_props_use_rear_middle_and_near_floor_contacts() {
    let props = [
        prop_topology(
            crate::game::habitat::TOKEN_PEBBLE_25K,
            0,
            PropZoneSnapshot::FloorLeft,
            AuthoredDepthSnapshot::Background,
        ),
        prop_topology(
            crate::game::habitat::TOKEN_SHELL_100K,
            1,
            PropZoneSnapshot::FloorRight,
            AuthoredDepthSnapshot::BehindPet,
        ),
        prop_topology(
            crate::game::habitat::TOKEN_MOSS_TUFT_250K,
            2,
            PropZoneSnapshot::FloorMid,
            AuthoredDepthSnapshot::Foreground,
        ),
    ];

    let contacts = props
        .iter()
        .map(|prop| {
            let composition = resolve_for(std::slice::from_ref(prop), 360.0, 360.0);
            let placement = composition.prop_placements[0];
            assert!(placement.visible);
            assert!(placement.grounded);
            placement.bounds_cells[3]
        })
        .collect::<Vec<_>>();

    assert_eq!(contacts, vec![15, 16, 17]);
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test grounded_props_use_rear_middle_and_near_floor_contacts
```

Expected: FAIL because all three current floor candidates resolve with a near-floor exclusive bottom bound of 17, producing `[17, 17, 17]` instead of `[15, 16, 17]`. Fix test compilation mistakes, but do not change production code until this behavioral failure is observed.

- [ ] **Step 3: Update stale single-row assertions to the approved contract**

Still before production changes, update the existing tests whose assertions encode the superseded single-row behavior:

```rust
// landscape_composition_uses_centered_aperture_columns
assert_eq!(placement.bounds_cells[3], 16);

// grounded_props_contact_the_tank_floor_without_changing_tank_reserves
for (catalog_id, expected_contact) in [
    (crate::game::habitat::TOKEN_PEBBLE_25K, 16),
    (crate::game::habitat::TOKEN_MOSS_TUFT_250K, 17),
] {
    // existing setup and safety assertions
    assert_eq!(placement.bounds_cells[3], expected_contact, "{catalog_id}");
}

// props_avoid_hud_and_bottom_reserves
assert!([15, 16, 17].contains(&placement.bounds_cells[3]));
```

Keep the bottom route reserve literal `[0, 13, width, 5]`, floor-HUD exclusion, aperture checks, and foreground reservation derivation unchanged. Rerun the focused test and confirm it remains RED for the same missing-lane reason.

- [ ] **Step 4: Add attachment and stability regression coverage**

Add tests using real catalog props and literal geometric relationships. These characterize existing non-floor behavior and protect it while the floor path changes.

```rust
#[test]
fn floor_lane_choice_ignores_stable_order_and_surface_shape() {
    let resolve_contact = |stable_order, width_points, height_points| {
        let props = [prop_topology(
            crate::game::habitat::TOKEN_PEBBLE_25K,
            stable_order,
            PropZoneSnapshot::FloorLeft,
            AuthoredDepthSnapshot::BehindPet,
        )];
        let placement = resolve_for(&props, width_points, height_points).prop_placements[0];
        assert!(placement.visible);
        placement.bounds_cells[3]
    };

    assert_eq!(resolve_contact(0, 360.0, 360.0), 16);
    assert_eq!(resolve_contact(7, 360.0, 360.0), 16);
    assert_eq!(resolve_contact(3, 480.0, 360.0), 16);
    assert_eq!(resolve_contact(5, 360.0, 480.0), 16);
}

#[test]
fn full_cast_grounded_props_do_not_collapse_to_one_floor_contact() {
    let props = full_prop_topology();
    let composition = resolve_for(&props, 360.0, 360.0);
    let contacts = composition
        .prop_placements
        .iter()
        .filter(|placement| placement.visible && placement.grounded)
        .map(|placement| placement.bounds_cells[3])
        .collect::<std::collections::BTreeSet<_>>();

    assert!(contacts.len() >= 2, "grounded props collapsed to {contacts:?}");
    assert!(contacts.iter().all(|contact| [15, 16, 17].contains(contact)));
}

#[test]
fn non_floor_props_keep_their_authored_attachment_regions() {
    let props = [
        prop_topology(
            crate::game::habitat::TOKEN_LANTERN_10M,
            0,
            PropZoneSnapshot::Ceiling,
            AuthoredDepthSnapshot::Background,
        ),
        prop_topology(
            crate::game::habitat::TOKEN_GEODE_50M,
            1,
            PropZoneSnapshot::WallLeft,
            AuthoredDepthSnapshot::BehindPet,
        ),
        prop_topology(
            crate::game::habitat::TOKEN_SHARD_1M,
            2,
            PropZoneSnapshot::WallRight,
            AuthoredDepthSnapshot::Background,
        ),
        prop_topology(
            crate::game::habitat::TOKEN_SPARK_500K,
            3,
            PropZoneSnapshot::AirLeft,
            AuthoredDepthSnapshot::Background,
        ),
    ];
    let composition = resolve_for(&props, 360.0, 360.0);
    let placements = &composition.prop_placements;

    assert!(placements.iter().all(|placement| placement.visible));
    assert!(placements.iter().all(|placement| !placement.grounded));
    assert!(placements[0].bounds_cells[1] <= 4, "ceiling prop detached");
    assert!(placements[1].bounds_cells[0] <= 4, "left-wall prop detached");
    assert!(placements[2].bounds_cells[2] >= 38, "right-wall prop detached");
    assert!(placements[3].bounds_cells[3] <= 13, "air prop entered floor band");
}
```

Run the non-floor test before production changes and confirm PASS; it is a baseline contract, not the TDD red test. Its literals come directly from the existing authored candidate boundaries and must not be derived with production helpers.

- [ ] **Step 5: Implement deterministic floor-lane candidate ordering**

Add the private lane type and deterministic catalog discriminator near `CandidateAnchor`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FloorDepthLane {
    Rear,
    Middle,
    Near,
}

impl FloorDepthLane {
    const fn bottom_offset(self) -> i16 {
        match self {
            Self::Rear => -3,
            Self::Middle => -2,
            Self::Near => -1,
        }
    }
}

fn stable_floor_lane_variant(catalog_id: &str) -> bool {
    const OFFSET: u64 = 1_469_598_103_934_665_603;
    const PRIME: u64 = 1_099_511_628_211;
    b"prop-floor-lane-v1|"
        .iter()
        .copied()
        .chain(catalog_id.bytes())
        .fold(OFFSET, |mut hash, byte| {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
            hash
        })
        .is_multiple_of(2)
}

fn floor_lane_order(prop: &PropTopologySnapshot) -> [FloorDepthLane; 3] {
    use FloorDepthLane::{Middle, Near, Rear};
    let alternate = stable_floor_lane_variant(prop.catalog_id);
    match (prop.authored_depth, alternate) {
        (AuthoredDepthSnapshot::Background, false) => [Rear, Middle, Near],
        (AuthoredDepthSnapshot::Background, true) => [Middle, Rear, Near],
        (AuthoredDepthSnapshot::BehindPet, false) => [Middle, Rear, Near],
        (AuthoredDepthSnapshot::BehindPet, true) => [Rear, Middle, Near],
        (AuthoredDepthSnapshot::Foreground, false) => [Near, Middle, Rear],
        (AuthoredDepthSnapshot::Foreground, true) => [Middle, Near, Rear],
    }
}
```

Keep the existing horizontal candidate vocabulary, but replace each grounded candidate's Y axis for every preferred lane:

```rust
fn grounded_candidate_anchors(
    prop: &PropTopologySnapshot,
    columns: u16,
    rows: u16,
    floor_hud_columns: [i16; 2],
) -> Vec<CandidateAnchor> {
    let mut horizontal = grounded_side_lane_anchors(prop.zone, rows, floor_hud_columns);
    horizontal.extend(candidate_anchors(prop.zone, columns, rows));
    let rows = i16::try_from(rows).unwrap_or(i16::MAX);

    floor_lane_order(prop)
        .into_iter()
        .flat_map(|lane| {
            horizontal.iter().copied().map(move |candidate| CandidateAnchor {
                x: candidate.x,
                y: CandidateAxis::End {
                    extent: rows,
                    offset: lane.bottom_offset(),
                },
            })
        })
        .collect()
}
```

Change the solver's candidate construction without changing any safety check:

```rust
let candidates = if grounded {
    grounded_candidate_anchors(
        prop,
        aperture_columns,
        candidate_rows,
        [floor_hud_reserve_local[0], floor_hud_reserve_local[2]],
    )
} else {
    candidate_anchors(prop.zone, aperture_columns, candidate_rows)
};
```

The code snippets are the intended minimal shape. If Rust borrow inference requires a small local collection or closure adjustment, keep the same private interfaces and ordering semantics rather than expanding scope.

- [ ] **Step 6: Verify GREEN and preserve projection behavior**

Run:

```bash
cargo test grounded_props_use_rear_middle_and_near_floor_contacts
cargo test floor_lane_choice_ignores_stable_order_and_surface_shape
cargo test full_cast_grounded_props_do_not_collapse_to_one_floor_contact
cargo test non_floor_props_keep_their_authored_attachment_regions
cargo test presentation::companion_scene::composition::tests
cargo test grounded_prop_active_sprite_stays_bottom_aligned_inside_frozen_footprint
cargo test --test retained_scene
```

Expected: all PASS. The existing active-footprint test must keep equal bottom edges across bloom states, proving animation still contacts its accepted lane. The retained integration suite must remain green without frame-slot or GPU ABI changes.

- [ ] **Step 7: Run formatting, static checks, and deterministic prop preview**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo run -- dev-preview --scenario props --out target/glorp-preview-prop-depth-lanes
test -f target/glorp-preview-prop-depth-lanes/manifest.json
```

Expected: formatting and Clippy exit 0 with clean output; Preview Lab exits 0 and produces the owned manifest. Do not open or fullscreen a UI during automated verification.

- [ ] **Step 8: Review the diff and commit**

Inspect only the intended source/test changes:

```bash
git diff --check
git diff -- src/presentation/companion_scene/composition.rs
git status --short
```

Commit:

```bash
git add src/presentation/companion_scene/composition.rs
git commit -m "fix(companion): distribute grounded props by depth"
```

The commit must not include generated Preview Lab output, unrelated working-tree changes, shader changes, or renderer ABI changes.
