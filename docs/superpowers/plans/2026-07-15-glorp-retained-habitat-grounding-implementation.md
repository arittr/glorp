# Glorp Retained Habitat Grounding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Keep grounded vegetation visibly inside the circular habitat, place the existing reeds on the rear floor, attach foreground vines to the real circular ceiling, and replace the flat retained bed with stable logical-scale granular texture.

**Architecture:** Refine the pure companion composition solver for attachment geometry and keep the shared habitat catalog authoritative for reeds depth. The retained room remains one analytic draw: its shader swaps physical-pixel noise for deterministic absolute logical-coordinate value noise and clustered marks, mirrored by the existing test-only CPU sampler. No scene ABI or GPU resource changes are required.

**Tech Stack:** Rust, WGSL, wgpu/Metal native readback tests, the existing retained companion scene, and Preview Lab.

## Global Constraints

- Grounded props retain their rear, middle, or near lane, keep at least one logical cell of horizontal inset from the point-space circular side rim, and use the full vertical aperture so they contact the floor. Canonical square contacts remain `15/16/17`; non-square contacts follow the actual circular floor.
- Grounded candidates move inward under competition and hide on exhaustion; they never expand toward the rim, change depth lanes, overlap the HUD, or lose the one-cell prop gutter.
- The existing reeds reward becomes shared authored `Background` depth; it is not duplicated and its identity, threshold, art, animation, color, zone, and priority do not change.
- Foreground ceiling attachment uses occupied prop cells against the full circular aperture. Empty rectangular footprint corners do not reject a valid vine, while background ceiling props remain gauge-safe and recessed.
- Substrate texture uses absolute logical tank coordinates, remains stable across frames and backing scales, and contributes nothing above the curved bed horizon.
- Keep the existing analytic room pass, biome bed/fleck paint lanes, prop nodes, Z ordering, parallax, contact shadows, pet floor silhouette, and aperture composite.
- Do not add a render pass, texture asset, bind group, uniform, buffer, pipeline, resize-owned resource, perspective camera, mesh floor, or dynamic per-frame prop packing.
- Implement each behavior test-first and record the intended RED failure before production edits.

---

### Task 1: Inset grounded props and move reeds to the rear floor

**Files:**
- Modify: `src/game/habitat.rs:285-301,748-775`
- Modify: `src/presentation/companion_scene/composition.rs:108-215,311-361,663-865`
- Modify: `tests/retained_scene.rs:373-610`

**Interfaces:**
- Consumes: `HabitatPropSpec::pet_layer`, `FloorDepthLane`, `grounded_side_lane_anchors`, `candidate_is_safe`, and the existing full-cast composition matrix.
- Produces: `FLOOR_APERTURE_INSET_CELLS: f32`, `aperture_radii_cells(CompanionCompositionInput) -> [f32; 2]`, `grounded_aperture_radii([f32; 2], f32) -> [f32; 2]`, `aperture_floor_extent_rows(u16, f32) -> i16`, inward floor-HUD candidates, and `TOKEN_REEDS_5M` with `HabitatPetLayer::Background`.
- Preserves: canonical square exclusive floor contacts `15`, `16`, and `17`, stable named depth lanes across surface shapes, all prop identities, and every non-floor placement rule.

- [ ] **Step 1: Add a failing inward-candidate test**

In `src/presentation/companion_scene/composition.rs`'s test module, add:

```rust
#[test]
fn grounded_side_lane_fallbacks_move_inward_from_the_hud() {
    let floor_hud_columns = [13, 31];
    let footprint = [1, 1];
    let left = grounded_side_lane_anchors(
        PropZoneSnapshot::FloorLeft,
        ROWS,
        floor_hud_columns,
    )
    .into_iter()
    .map(|candidate| candidate.resolve(footprint)[0])
    .collect::<Vec<_>>();
    let right = grounded_side_lane_anchors(
        PropZoneSnapshot::FloorRight,
        ROWS,
        floor_hud_columns,
    )
    .into_iter()
    .map(|candidate| candidate.resolve(footprint)[0])
    .collect::<Vec<_>>();

    assert_eq!(left, vec![12, 14, 16]);
    assert_eq!(right, vec![31, 29, 27]);
}
```

- [ ] **Step 2: Run the focused tests and verify RED**

Run:

```bash
cargo test --lib grounded_side_lane_fallbacks_move_inward_from_the_hud
```

Expected: the candidate test reports left positions `[12, 10, 8]` and right positions `[31, 33, 35]`. The composition matrix in Step 3 proves the reeds depth contract through the real placement consumer.

- [ ] **Step 3: Add the failing inset/depth composition matrix**

Add this test beside the existing floor-lane tests in `composition.rs`:

```rust
#[test]
fn moss_and_reeds_keep_an_inset_and_separate_floor_depths() {
    for &(width_points, height_points) in SURFACES {
        let props = [
            crate::game::habitat::TOKEN_MOSS_TUFT_250K,
            crate::game::habitat::TOKEN_REEDS_5M,
        ]
        .into_iter()
        .enumerate()
        .map(|(stable_order, catalog_id)| {
            let spec = crate::game::habitat::catalog_prop_by_str(catalog_id)
                .expect("ground vegetation catalog entry");
            prop_topology(
                spec.id,
                u8::try_from(stable_order).unwrap(),
                spec.zone.into(),
                spec.pet_layer.into(),
            )
        })
        .collect::<Vec<_>>();
        let composition = resolve_for(&props, width_points, height_points);
        let aperture_radius_points = width_points.min(height_points) / 2.0;
        let full_radii = [
            aperture_radius_points / (width_points / f32::from(COLUMNS)),
            aperture_radius_points / (height_points / f32::from(ROWS)),
        ];
        let radii = [full_radii[0] - 1.0, full_radii[1]];
        let center = [f32::from(COLUMNS) / 2.0, f32::from(ROWS) / 2.0];
        let floor_extent = (center[1] + full_radii[1] + 0.5)
            .floor()
            .clamp(1.0, f32::from(ROWS)) as i16;

        assert_eq!(composition.prop_placements.len(), 2);
        for placement in &composition.prop_placements {
            assert!(placement.visible, "{width_points}x{height_points}");
            assert!(placement.grounded);
            for col in [placement.bounds_cells[0], placement.bounds_cells[2] - 1] {
                for row in [placement.bounds_cells[1], placement.bounds_cells[3] - 1] {
                    let dx = (f32::from(col) + 0.5 - center[0]) / radii[0];
                    let dy = (f32::from(row) + 0.5 - center[1]) / radii[1];
                    assert!(
                        dx * dx + dy * dy <= 1.0,
                        "{width_points}x{height_points} clipped {placement:?}",
                    );
                }
            }
        }
        assert_eq!(composition.prop_placements[0].bounds_cells[3], floor_extent - 1);
        assert_eq!(composition.prop_placements[1].bounds_cells[3], floor_extent - 3);
    }
}
```

Run:

```bash
cargo test --lib moss_and_reeds_keep_an_inset_and_separate_floor_depths
```

Expected: FAIL because the catalog still puts reeds in the near lane, outward candidates violate the inset, and tall surfaces still derive Y contacts from the rectangular window bottom.

- [ ] **Step 4: Implement the shared rear-depth reeds contract**

Change only the reeds catalog entry in `src/game/habitat.rs`:

```rust
HabitatPropSpec {
    id: TOKEN_REEDS_5M,
    kind: HabitatPropKind::Trophy,
    zone: HabitatPropZone::FloorRight,
    display_priority: 151,
    lifetime_threshold: Some(5_000_000.0),
    pet_layer: HabitatPetLayer::Background,
    color: (0x8c, 0xc4, 0x6c),
},
```

Do not add reeds to `flowering_plants_are_foreground_layer`; moss, vine, and planter retain that foreground contract.

- [ ] **Step 5: Implement inward candidates and the inset floor aperture**

Add near `FloorDepthLane` in `composition.rs`:

```rust
const FLOOR_APERTURE_INSET_CELLS: f32 = 1.0;

fn aperture_radii_cells(input: CompanionCompositionInput<'_>) -> [f32; 2] {
    if input.columns == 0
        || input.rows == 0
        || !input.width_points.is_finite()
        || !input.height_points.is_finite()
        || input.width_points <= 0.0
        || input.height_points <= 0.0
    {
        return [0.0; 2];
    }
    let radius_points = input.width_points.min(input.height_points) / 2.0;
    [
        radius_points / (input.width_points / f32::from(input.columns)),
        radius_points / (input.height_points / f32::from(input.rows)),
    ]
}

fn grounded_aperture_radii(radii: [f32; 2], horizontal_inset_cells: f32) -> [f32; 2] {
    [(radii[0] - horizontal_inset_cells).max(0.0), radii[1]]
}

fn aperture_floor_extent_rows(rows: u16, radius_rows: f32) -> i16 {
    (f32::from(rows) / 2.0 + radius_rows + 0.5)
        .floor()
        .clamp(1.0, f32::from(rows)) as i16
}
```

Replace the two offset arrays in `grounded_side_lane_anchors`:

```rust
let left = [0, 2, 4].map(|offset| CandidateAnchor {
    x: CandidateAxis::End {
        extent: floor_hud_columns[0],
        offset,
    },
    y: grounded_y,
});
let right = [0, -2, -4].map(|offset| CandidateAnchor {
    x: CandidateAxis::Start(floor_hud_columns[1].saturating_add(offset)),
    y: grounded_y,
});
```

In `resolve_companion_composition`, derive both axes from point geometry, then add the inset and actual circular floor extent:

```rust
let aperture_radius_cells = aperture_radii_cells(input);
let grounded_radius_cells =
    grounded_aperture_radii(aperture_radius_cells, FLOOR_APERTURE_INSET_CELLS);
let floor_extent_rows = aperture_floor_extent_rows(input.rows, aperture_radius_cells[1]);
```

Add `floor_extent_rows: i16` to `grounded_candidate_anchors` and use it as the `CandidateAxis::End` extent for the lane Y:

```rust
fn grounded_candidate_anchors(
    prop: &PropTopologySnapshot,
    columns: u16,
    rows: u16,
    floor_extent_rows: i16,
    floor_hud_columns: [i16; 2],
) -> Vec<CandidateAnchor> {
    let mut horizontal = grounded_side_lane_anchors(prop.zone, rows, floor_hud_columns);
    horizontal.extend(candidate_anchors(
        prop.zone,
        prop.authored_depth,
        columns,
        rows,
    ));
    let lane = floor_lane(prop);
    horizontal
        .into_iter()
        .map(|candidate| CandidateAnchor {
            x: candidate.x,
            y: CandidateAxis::End {
                extent: floor_extent_rows,
                offset: lane.bottom_offset(),
            },
        })
        .collect()
}
```

Pass `floor_extent_rows` from `resolve_companion_composition`. Pass `grounded_radius_cells` instead of `aperture_radius_cells` to `candidate_is_safe` when `grounded` is true. Keep the generic zone candidates after the side-lane candidates; the inset safety check rejects any generic candidate that approaches the rim.

Update `floor_lane_choice_ignores_stable_order_and_surface_shape` so it compares `floor_extent_rows - placement.bounds_cells[3]` across shapes, which remains the same named-lane offset, rather than requiring the same absolute row in a letterboxed circle. Make the same relative-offset correction in `same_lane_competition_never_changes_an_accepted_contact`.

In `full_cast_props_are_disjoint_and_inside_safe_aperture`, replace the grounded `safe_radii` branch with the point-derived inset radii:

```rust
let aperture_radius_points = width_points.min(height_points) / 2.0;
let safe_radii = if placement.grounded {
    [
        aperture_radius_points / (width_points / f32::from(COLUMNS)) - 1.0,
        aperture_radius_points / (height_points / f32::from(ROWS)),
    ]
} else {
    expected_radii
};
```

In `props_avoid_hud_and_bottom_reserves`, derive the three accepted grounded contacts for each surface and replace the literal `[15, 16, 17]`:

```rust
let aperture_radius_rows = width_points.min(height_points) / 2.0
    / (height_points / f32::from(ROWS));
let floor_extent = (f32::from(ROWS) / 2.0 + aperture_radius_rows + 0.5)
    .floor()
    .clamp(1.0, f32::from(ROWS)) as i16;
let floor_contacts = [floor_extent - 3, floor_extent - 2, floor_extent - 1];
assert!(floor_contacts.contains(&placement.bounds_cells[3]));
```

Add this exhaustion regression beside `same_lane_competition_never_changes_an_accepted_contact`:

```rust
#[test]
fn same_lane_competition_hides_after_inset_candidates_are_exhausted() {
    let fixtures = [
        crate::game::habitat::TOKEN_PEBBLE_25K,
        crate::game::habitat::TOKEN_SHELL_100K,
        crate::game::habitat::TOKEN_MOSS_TUFT_250K,
        crate::game::habitat::TOKEN_SHARD_1M,
        crate::game::habitat::TOKEN_TREASURE_CHEST_2M,
        crate::game::habitat::TOKEN_ORBIT_5M,
        crate::game::habitat::TOKEN_REEDS_5M,
        crate::game::habitat::WILT_RECOVERY_SPROUT,
    ];
    let props = fixtures
        .into_iter()
        .enumerate()
        .map(|(stable_order, catalog_id)| {
            prop_topology(
                catalog_id,
                u8::try_from(stable_order).unwrap(),
                PropZoneSnapshot::FloorLeft,
                AuthoredDepthSnapshot::Foreground,
            )
        })
        .collect::<Vec<_>>();
    let composition = resolve_for(&props, 360.0, 360.0);
    let visible = composition
        .prop_placements
        .iter()
        .filter(|placement| placement.visible)
        .collect::<Vec<_>>();

    assert!(visible.len() < props.len(), "competition did not exhaust candidates");
    assert!(visible
        .iter()
        .all(|placement| placement.bounds_cells[3] == 17));
}
```

- [ ] **Step 6: Strengthen the retained full-cast integration assertion**

In `tests/retained_scene.rs::retained_full_cast_composition_matrix`, change only the grounded safe radii:

```rust
let prop_safe_radii = if grounded {
    [
        aperture_radius as f32 / cell[0] - 1.0,
        aperture_radius as f32 / cell[1],
    ]
} else {
    safe_radii
};
```

Replace the literal grounded contacts with `floor_extent - 3.0`, `floor_extent - 2.0`, and `floor_extent - 1.0`, where `floor_extent = (ROWS as f32 / 2.0 + aperture_radius as f32 / cell[1] + 0.5).floor()`. Keep the floor HUD rectangle, bottom reserve, pairwise non-overlap, repeated-projection equality, and phase-change assertions unchanged.

- [ ] **Step 7: Run GREEN checks and commit Task 1**

Run:

```bash
cargo test --lib grounded_side_lane_fallbacks_move_inward_from_the_hud
cargo test --lib moss_and_reeds_keep_an_inset_and_separate_floor_depths
cargo test --test retained_scene retained_full_cast_composition_matrix
cargo test --lib presentation::companion_scene::composition::tests
```

Expected: all focused tests PASS; on square surfaces moss/reeds end on `17/15`, on tall surfaces their named lane offsets follow the circular floor, and every accepted ground footprint satisfies the point-correct horizontal side inset while contacting the full vertical floor aperture.

Commit:

```bash
git add src/game/habitat.rs src/presentation/companion_scene/composition.rs tests/retained_scene.rs
git commit -m "fix(companion): inset grounded vegetation"
```

---

### Task 2: Attach foreground ceiling props by occupied cells

**TDD sequencing note:** Perform Step 3's contour-contact test and observe its
behavioral RED before Steps 1-2. Then add the occupied-offset helper and its
focused unit coverage as part of the smallest GREEN implementation. A
missing-symbol compiler error does not count as the RED result.

**Files:**
- Modify: `src/presentation/props.rs:22-35,394-474,565-640`
- Modify: `src/presentation/companion_scene/composition.rs:139-215,389-487,920-957`
- Modify: `src/companion/retained/render.rs:9904-10034`
- Modify: `tests/retained_scene.rs:373-610`

**Interfaces:**
- Consumes: the union of canonical sprite states, `PresentationPropFootprint`, foreground authored depth, and the full circular `aperture_radius_cells`.
- Produces: `presentation_prop_occupied_offsets(&str) -> Option<Vec<[i8; 2]>>`, `cell_inside_ellipse`, `occupied_cells_inside_ellipse`, and `highest_safe_ceiling_anchor_row`.
- Preserves: rectangular bounds for collision/HUD/gutter checks, gauge-safe recessed placement for background ceiling props, and all wall/air/floor behavior.

- [ ] **Step 1: Add a failing occupied-offset union test**

Add to `src/presentation/props.rs`'s test module:

```rust
#[test]
fn vine_occupied_offsets_cover_every_animation_state() {
    assert_eq!(
        presentation_prop_occupied_offsets(TOKEN_HANGING_VINE_25M).unwrap(),
        vec![[0, 0], [0, 2], [1, 0], [1, 1], [1, 2], [2, 0], [2, 2]],
    );
}
```

Run:

```bash
cargo test --lib vine_occupied_offsets_cover_every_animation_state
```

Expected: compilation fails because `presentation_prop_occupied_offsets` does not exist. This is the intended interface RED.

- [ ] **Step 2: Implement and pass the occupied-offset helper**

Add beside `presentation_prop_max_footprint`:

```rust
pub(crate) fn presentation_prop_occupied_offsets(
    catalog_id: &str,
) -> Option<Vec<[i8; 2]>> {
    let mut offsets = Vec::new();
    for state in presentation_prop_visual_states(catalog_id)? {
        for cell in presentation_prop_sprite(catalog_id, state)? {
            let offset = [cell.dx, cell.dy];
            if !offsets.contains(&offset) {
                offsets.push(offset);
            }
        }
    }
    offsets.sort_unstable();
    Some(offsets)
}
```

Run the helper test again and expect PASS.

- [ ] **Step 3: Replace the fixed-row vine test with a failing contour-contact test**

Replace `foreground_ceiling_props_contact_the_top_while_background_props_stay_recessed` in `composition.rs` with:

```rust
#[test]
fn foreground_ceiling_props_contact_the_aperture_by_occupied_cells() {
    let foreground_vine = prop_topology(
        crate::game::habitat::TOKEN_HANGING_VINE_25M,
        0,
        PropZoneSnapshot::Ceiling,
        AuthoredDepthSnapshot::Foreground,
    );
    let background_lantern = prop_topology(
        crate::game::habitat::TOKEN_LANTERN_10M,
        0,
        PropZoneSnapshot::Ceiling,
        AuthoredDepthSnapshot::Background,
    );
    let occupied = crate::presentation::props::presentation_prop_occupied_offsets(
        crate::game::habitat::TOKEN_HANGING_VINE_25M,
    )
    .unwrap();

    let lantern =
        resolve_for(std::slice::from_ref(&background_lantern), 360.0, 360.0)
            .prop_placements[0];
    assert!(lantern.visible);
    assert_eq!(lantern.bounds_cells[1], 4);

    for &(width_points, height_points) in SURFACES {
        let vine = resolve_for(
            std::slice::from_ref(&foreground_vine),
            width_points,
            height_points,
        )
        .prop_placements[0];
        let aperture_radius_points = width_points.min(height_points) / 2.0;
        let radii = [
            aperture_radius_points / (width_points / f32::from(COLUMNS)),
            aperture_radius_points / (height_points / f32::from(ROWS)),
        ];

        assert!(vine.visible, "{width_points}x{height_points}");
        assert!(occupied_cells_inside_ellipse(
            vine.anchor_cell,
            &occupied,
            i16::try_from(COLUMNS).unwrap(),
            i16::try_from(ROWS).unwrap(),
            radii,
        ));
        assert!(!occupied_cells_inside_ellipse(
            [vine.anchor_cell[0], vine.anchor_cell[1] - 1],
            &occupied,
            i16::try_from(COLUMNS).unwrap(),
            i16::try_from(ROWS).unwrap(),
            radii,
        ));
    }
}
```

Run:

```bash
cargo test --lib foreground_ceiling_props_contact_the_aperture_by_occupied_cells
```

Expected: compilation first fails until the private ellipse helper is added; after adding only the helper, the behavioral assertion still fails because the current vine at row `2` or `4` can move upward and remain inside the full aperture.

- [ ] **Step 4: Add occupied-cell ellipse helpers**

Refactor the existing ellipse check in `composition.rs` to use these helpers:

```rust
fn cell_inside_ellipse(
    col: i16,
    row: i16,
    columns: i16,
    rows: i16,
    radii: [f32; 2],
) -> bool {
    if col < 0 || row < 0 || col >= columns || row >= rows || radii[0] <= 0.0 || radii[1] <= 0.0 {
        return false;
    }
    let center = [f32::from(columns) / 2.0, f32::from(rows) / 2.0];
    let dx = (f32::from(col) + 0.5 - center[0]) / radii[0];
    let dy = (f32::from(row) + 0.5 - center[1]) / radii[1];
    dx * dx + dy * dy <= 1.0
}

fn occupied_cells_inside_ellipse(
    anchor_cell: [i16; 2],
    occupied_offsets: &[[i8; 2]],
    columns: i16,
    rows: i16,
    radii: [f32; 2],
) -> bool {
    !occupied_offsets.is_empty()
        && occupied_offsets.iter().all(|[dx, dy]| {
            cell_inside_ellipse(
                anchor_cell[0] + i16::from(*dx),
                anchor_cell[1] + i16::from(*dy),
                columns,
                rows,
                radii,
            )
        })
}

fn highest_safe_ceiling_anchor_row(
    anchor_x: i16,
    footprint: crate::presentation::props::PresentationPropFootprint,
    occupied_offsets: &[[i8; 2]],
    columns: i16,
    rows: i16,
    radii: [f32; 2],
) -> Option<i16> {
    let first = -i16::from(footprint.min_dy);
    let last = rows
        .saturating_sub(1)
        .saturating_sub(i16::from(footprint.max_dy));
    (first..=last).find(|anchor_y| {
        occupied_cells_inside_ellipse(
            [anchor_x, *anchor_y],
            occupied_offsets,
            columns,
            rows,
            radii,
        )
    })
}

fn bounds_inside_ellipse(bounds: [i16; 4], columns: i16, rows: i16, radii: [f32; 2]) -> bool {
    [bounds[0], bounds[2] - 1].into_iter().all(|col| {
        [bounds[1], bounds[3] - 1]
            .into_iter()
            .all(|row| cell_inside_ellipse(col, row, columns, rows, radii))
    })
}
```

Split the non-ellipse clauses from `candidate_is_safe` into `candidate_regions_are_clear` so both rectangular and occupied-cell aperture policies reuse the same viewport, HUD, bottom-reserve, and collision checks.

```rust
fn candidate_regions_are_clear(
    bounds: [i16; 4],
    columns: i16,
    rows: i16,
    hud_reserve: [i16; 4],
    bottom_reserve: [i16; 4],
    accepted_bounds: &[[i16; 4]],
) -> bool {
    bounds[0] >= 0
        && bounds[1] >= 0
        && bounds[2] <= columns
        && bounds[3] <= rows
        && bounds[0] < bounds[2]
        && bounds[1] < bounds[3]
        && !intersects(bounds, hud_reserve)
        && !intersects(bounds, bottom_reserve)
        && accepted_bounds
            .iter()
            .all(|accepted| !intersects(bounds, expand(*accepted)))
}

fn candidate_is_safe(
    bounds: [i16; 4],
    columns: i16,
    rows: i16,
    radii: [f32; 2],
    hud_reserve: [i16; 4],
    bottom_reserve: [i16; 4],
    accepted_bounds: &[[i16; 4]],
) -> bool {
    bounds_inside_ellipse(bounds, columns, rows, radii)
        && candidate_regions_are_clear(
            bounds,
            columns,
            rows,
            hud_reserve,
            bottom_reserve,
            accepted_bounds,
        )
}
```

- [ ] **Step 5: Resolve foreground ceiling Y from the circular contour**

For foreground `PropZoneSnapshot::Ceiling`, reduce `candidate_anchors` to horizontal choices at nominal row zero:

```rust
vec![
    anchor(center_x(0), start_y(0)),
    anchor(center_x(-8), start_y(0)),
    anchor(center_x(8), start_y(0)),
]
```

In `resolve_companion_composition`, derive once per prop:

```rust
let foreground_ceiling = prop.zone == PropZoneSnapshot::Ceiling
    && prop.authored_depth == AuthoredDepthSnapshot::Foreground;
let occupied_offsets = if foreground_ceiling {
    crate::presentation::props::presentation_prop_occupied_offsets(prop.catalog_id)
        .unwrap_or_default()
} else {
    Vec::new()
};
```

Change the candidate iterator from `map` to `filter_map`. After adding `aperture_start_column`, replace the nominal Y only for foreground ceiling props:

```rust
if foreground_ceiling {
    let anchor_x = top_left[0] - i16::from(footprint.min_dx);
    let anchor_y = highest_safe_ceiling_anchor_row(
        anchor_x,
        footprint,
        &occupied_offsets,
        columns,
        rows,
        aperture_radius_cells,
    )?;
    top_left[1] = anchor_y + i16::from(footprint.min_dy);
}
Some(top_left)
```

After calculating `anchor_cell` and `bounds`, choose the aperture predicate:

```rust
let aperture_safe = if foreground_ceiling {
    occupied_cells_inside_ellipse(
        anchor_cell,
        &occupied_offsets,
        columns,
        rows,
        aperture_radius_cells,
    )
} else {
    bounds_inside_ellipse(
        bounds,
        columns,
        rows,
        if grounded {
            grounded_radius_cells
        } else {
            gauge_inner_radius_cells
        },
    )
};
let safe = aperture_safe
    && candidate_regions_are_clear(
        bounds,
        columns,
        rows,
        candidate_hud_reserve,
        candidate_bottom_reserve,
        &accepted_bounds,
    );
safe.then_some((anchor_cell, bounds))
```

An empty occupied-offset set therefore fails closed. Background ceiling candidates continue through their existing row-4-first list and gauge-inner rectangular safety.

- [ ] **Step 6: Update the full-cast integration policy for foreground ceiling props**

In `composition.rs::full_cast_props_are_disjoint_and_inside_safe_aperture`, replace the unconditional corner loop with the same narrow exception:

```rust
let foreground_ceiling = topology.zone == PropZoneSnapshot::Ceiling
    && topology.authored_depth == AuthoredDepthSnapshot::Foreground;
if foreground_ceiling {
    let occupied = crate::presentation::props::presentation_prop_occupied_offsets(
        topology.catalog_id,
    )
    .unwrap();
    let aperture_radius_points = width_points.min(height_points) / 2.0;
    let aperture_radii = [
        aperture_radius_points / (width_points / f32::from(COLUMNS)),
        aperture_radius_points / (height_points / f32::from(ROWS)),
    ];
    assert!(occupied_cells_inside_ellipse(
        placement.anchor_cell,
        &occupied,
        i16::try_from(COLUMNS).unwrap(),
        i16::try_from(ROWS).unwrap(),
        aperture_radii,
    ));
} else {
    for col in [f32::from(min_col) + 0.5, f32::from(max_col) - 0.5] {
        for row in [f32::from(min_row) + 0.5, f32::from(max_row) - 0.5] {
            let dx = (col - center[0]) / safe_radii[0];
            let dy = (row - center[1]) / safe_radii[1];
            assert!(dx * dx + dy * dy <= 1.0 + f32::EPSILON);
        }
    }
}
```

Import `AuthoredDepthSnapshot` in `retained_full_cast_composition_matrix`. Before the existing four-corner ellipse loop, add:

```rust
let foreground_ceiling = prop.zone == PropZoneSnapshot::Ceiling
    && prop.authored_depth == AuthoredDepthSnapshot::Foreground;
if foreground_ceiling {
    let aperture_top_row = (f32::from(ROWS) / 2.0
        - aperture_radius as f32 / cell[1]
        - 0.5)
        .ceil()
        .max(0.0);
    assert_eq!(
        bounds[1], aperture_top_row,
        "{label} foreground ceiling slot {} did not meet the aperture top",
        frame.slot,
    );
} else {
    for col in [bounds[0] + 0.5, bounds[2] - 0.5] {
        for row in [bounds[1] + 0.5, bounds[3] - 0.5] {
            let dx = (col - center[0]) / prop_safe_radii[0];
            let dy = (row - center[1]) / prop_safe_radii[1];
            assert!(dx * dx + dy * dy <= 1.0 + f32::EPSILON);
        }
    }
}
```

Remove the superseded unconditional four-corner loop. The focused unit test owns occupied-cell contour proof; this external integration test proves that the projected foreground ceiling origin reaches the point-correct circular top (row zero on square views and the letterboxed top row on tall views) without weakening any other prop's rectangle checks.

Apply the same policy in `render.rs::retained_full_cast_rois_are_nonblank_at_one_and_two_x`. Clone `snapshot.topology.visible_props` into `prop_topology` before moving `snapshot` into `compile_retained_full_cast_snapshot`. Resolve the topology prop for each `slot`, detect foreground ceiling, and bypass only its rectangular corner-to-gauge assertion:

```rust
let prop = prop_topology
    .iter()
    .find(|prop| prop.stable_order == *slot)
    .expect("topology for visible prop");
let foreground_ceiling = prop.zone == PropZoneSnapshot::Ceiling
    && prop.authored_depth == AuthoredDepthSnapshot::Foreground;
if foreground_ceiling {
    let aperture_radius_rows = layout.width_points.min(layout.height_points) / 2.0
        / grid.cell_extent_points[1];
    let expected_top = (f32::from(grid.rows) / 2.0 - aperture_radius_rows - 0.5)
        .ceil()
        .max(0.0) as i16;
    assert_eq!(placement.bounds_cells[1], expected_top);
} else {
    for x in safe_x {
        for y in safe_y {
            let dx = (x - gauge_center[0]) / safe_radius;
            let dy = (y - gauge_center[1]) / safe_radius;
            assert!(dx * dx + dy * dy <= 1.0);
        }
    }
}
```

Import `AuthoredDepthSnapshot` and `PropZoneSnapshot` into that native test. Keep its 1x/2x nonblank ROI assertions unchanged.

- [ ] **Step 7: Run GREEN checks and commit Task 2**

Run:

```bash
cargo test --lib vine_occupied_offsets_cover_every_animation_state
cargo test --lib foreground_ceiling_props_contact_the_aperture_by_occupied_cells
cargo test --lib non_floor_props_keep_their_authored_attachment_regions
cargo test --test retained_scene retained_full_cast_composition_matrix
cargo test --lib --features retained-renderer retained_full_cast_rois_are_nonblank_at_one_and_two_x
cargo test --lib presentation::companion_scene::composition::tests
```

Expected: all PASS; the vine cannot move one row higher, the lantern remains at row `4`, and wall/air fixtures retain their existing bounds.

Commit:

```bash
git add src/presentation/props.rs src/presentation/companion_scene/composition.rs src/companion/retained/render.rs tests/retained_scene.rs
git commit -m "fix(companion): attach vines to the ceiling"
```

---

### Task 3: Replace physical-pixel bed noise with logical granular texture

**Files:**
- Modify: `src/presentation/companion_effects.rs:107-177,300-370`
- Modify: `src/companion/retained/scene.wgsl:809-864`
- Modify: `src/companion/retained/render.rs:8721-8756,9798-9900`
- Verify unchanged: `src/presentation/companion_scene/scene.rs:964-984`
- Verify unchanged: `src/companion/retained/compiler.rs:2080-2105`

**Interfaces:**
- Consumes: absolute `point_y_down`, `bed_mix`, and existing packed biome bed/fleck colors.
- Produces: `substrate_hash01`, `substrate_value_noise`, `substrate_mark`, and CPU-mirrored `broad_tone_levels`, `grain_mix`, and `fleck_mix`.
- Preserves: `AnalyticPaint::ApertureDepth`, payload packing, checksums, validation, and the single analytic room draw.

- [ ] **Step 1: Add the deterministic backing-scale RED test**

Replace `bed_texture_is_stable_for_same_logical_sample` in `companion_effects.rs` with:

```rust
#[test]
fn bed_texture_is_invariant_to_backing_scale() {
    let at_1x = bed_texture_sample([144.5, 300.5], [360.0; 2], 1.0, "starter");
    let at_2x = bed_texture_sample([144.5, 300.5], [360.0; 2], 2.0, "starter");
    assert_eq!(at_1x, at_2x);
    assert!(at_1x.bed_mix > 0.6);
}
```

Run:

```bash
cargo test --lib --features retained-renderer bed_texture_is_invariant_to_backing_scale
```

Expected: FAIL. Current measured values at that logical point are `dither_levels=0.2611, fleck_mix=0.7217` at 1x and `dither_levels=-0.5721, fleck_mix=0.0` at 2x.

- [ ] **Step 2: Implement the test-only CPU mirror in logical coordinates**

Replace `BedTextureSample` with:

```rust
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BedTextureSample {
    pub(crate) bed_mix: f32,
    pub(crate) broad_tone_levels: f32,
    pub(crate) grain_mix: f32,
    pub(crate) fleck_mix: f32,
    pub(crate) bed_srgb8: [u8; 3],
    pub(crate) fleck_srgb8: [u8; 3],
}
```

Add these test-only helpers and use the same constants in WGSL:

```rust
#[cfg(test)]
fn substrate_hash01(cell: [u32; 2], salt: u32) -> f32 {
    let mut hash = cell[0].wrapping_mul(0x9E37_79B9)
        ^ cell[1].wrapping_mul(0x85EB_CA6B)
        ^ salt;
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x7FEB_352D);
    hash ^= hash >> 15;
    (hash & 0xFFFF) as f32 / 65535.0
}

#[cfg(test)]
fn texture_smooth_step(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
fn substrate_value_noise(point: [f32; 2], cell_size: f32, salt: u32) -> f32 {
    let grid = [point[0] / cell_size, point[1] / cell_size];
    let base = [grid[0].floor() as u32, grid[1].floor() as u32];
    let fraction = [grid[0].fract(), grid[1].fract()];
    let weight = [
        fraction[0] * fraction[0] * (3.0 - 2.0 * fraction[0]),
        fraction[1] * fraction[1] * (3.0 - 2.0 * fraction[1]),
    ];
    let n00 = substrate_hash01(base, salt);
    let n10 = substrate_hash01([base[0] + 1, base[1]], salt);
    let n01 = substrate_hash01([base[0], base[1] + 1], salt);
    let n11 = substrate_hash01([base[0] + 1, base[1] + 1], salt);
    let top = n00 + (n10 - n00) * weight[0];
    let bottom = n01 + (n11 - n01) * weight[0];
    top + (bottom - top) * weight[1]
}

#[cfg(test)]
fn substrate_mark(
    point: [f32; 2],
    cell_size: f32,
    radius: f32,
    density: f32,
    salt: u32,
) -> f32 {
    let grid = [point[0] / cell_size, point[1] / cell_size];
    let cell = [grid[0].floor() as u32, grid[1].floor() as u32];
    if substrate_hash01(cell, salt) >= density {
        return 0.0;
    }
    let center_span = (cell_size - 2.0 * radius).max(0.0);
    let center = [
        radius + substrate_hash01(cell, salt ^ 0xA511_E9B3) * center_span,
        radius + substrate_hash01(cell, salt ^ 0x63D8_35A7) * center_span,
    ];
    let local = [grid[0].fract() * cell_size, grid[1].fract() * cell_size];
    let distance = ((local[0] - center[0]).powi(2) + (local[1] - center[1]).powi(2)).sqrt();
    1.0 - texture_smooth_step(radius - 0.75, radius + 0.75, distance)
}
```

Replace the physical hash inside `bed_texture_sample` with:

```rust
let texture_gate = texture_smooth_step(0.18, 0.82, bed_mix);
let broad_tone_levels =
    (substrate_value_noise(logical_point_y_down, 36.0, 0xC13F_A9A9) - 0.5)
        * 14.0
        * texture_gate;
let grain_mix = substrate_mark(
    logical_point_y_down,
    10.0,
    1.6,
    0.50,
    0x91E1_0DA5,
) * 0.22
    * texture_gate;
let fleck_mix = substrate_mark(
    logical_point_y_down,
    30.0,
    2.8,
    0.28,
    0xD1B5_4A35,
) * 0.48
    * texture_gate;
let _ = backing_scale;
```

Return those three texture fields. The backing scale remains in the test helper signature solely to prove that it has no effect.

- [ ] **Step 3: Strengthen CPU grain and upper-room tests**

Add:

```rust
#[test]
fn bed_texture_has_broad_tone_and_clustered_marks() {
    let samples = (276..=348)
        .step_by(2)
        .flat_map(|y| {
            (48..=312).step_by(2).map(move |x| {
                bed_texture_sample([x as f32 + 0.5, y as f32 + 0.5], [360.0; 2], 1.0, "starter")
            })
        })
        .collect::<Vec<_>>();
    assert!(samples.iter().any(|sample| sample.broad_tone_levels.abs() >= 2.0));
    assert!(samples.iter().any(|sample| sample.grain_mix >= 0.10));
    assert!(samples.iter().any(|sample| sample.fleck_mix >= 0.20));
}
```

Update the upper-room loop to assert all substrate terms are zero:

```rust
assert_eq!(sample.bed_mix, 0.0, "sample=({x}, {y})");
assert_eq!(sample.broad_tone_levels, 0.0, "sample=({x}, {y})");
assert_eq!(sample.grain_mix, 0.0, "sample=({x}, {y})");
assert_eq!(sample.fleck_mix, 0.0, "sample=({x}, {y})");
```

Keep `bed_texture_changes_with_biome`, replacing its old texture comparisons with:

```rust
assert_eq!(starter.bed_mix, botanical.bed_mix);
assert_eq!(starter.broad_tone_levels, botanical.broad_tone_levels);
assert_eq!(starter.grain_mix, botanical.grain_mix);
assert_eq!(starter.fleck_mix, botanical.fleck_mix);
assert_eq!(starter.bed_srgb8, [72, 83, 108]);
assert_eq!(starter.fleck_srgb8, [60, 65, 80]);
assert_eq!(botanical.bed_srgb8, [63, 111, 102]);
assert_eq!(botanical.fleck_srgb8, [55, 81, 76]);
```

- [ ] **Step 4: Mirror the analytic texture in WGSL**

Add these WGSL mirrors immediately before `fs_room_aperture`:

```wgsl
fn substrate_hash01(cell: vec2<u32>, salt: u32) -> f32 {
    var hash = (cell.x * 0x9e3779b9u) ^ (cell.y * 0x85ebca6bu) ^ salt;
    hash = hash ^ (hash >> 16u);
    hash = hash * 0x7feb352du;
    hash = hash ^ (hash >> 15u);
    return f32(hash & 0xffffu) / 65535.0;
}

fn substrate_value_noise(point: vec2<f32>, cell_size: f32, salt: u32) -> f32 {
    let grid = max(point / cell_size, vec2<f32>(0.0));
    let base = vec2<u32>(floor(grid));
    let fraction = fract(grid);
    let weight = fraction * fraction * (vec2<f32>(3.0) - 2.0 * fraction);
    let n00 = substrate_hash01(base, salt);
    let n10 = substrate_hash01(base + vec2<u32>(1u, 0u), salt);
    let n01 = substrate_hash01(base + vec2<u32>(0u, 1u), salt);
    let n11 = substrate_hash01(base + vec2<u32>(1u, 1u), salt);
    return mix(mix(n00, n10, weight.x), mix(n01, n11, weight.x), weight.y);
}

fn substrate_mark(
    point: vec2<f32>,
    cell_size: f32,
    radius: f32,
    density: f32,
    salt: u32,
) -> f32 {
    let grid = max(point / cell_size, vec2<f32>(0.0));
    let cell = vec2<u32>(floor(grid));
    if (substrate_hash01(cell, salt) >= density) {
        return 0.0;
    }
    let center_span = max(cell_size - 2.0 * radius, 0.0);
    let center = vec2<f32>(
        radius + substrate_hash01(cell, salt ^ 0xa511e9b3u) * center_span,
        radius + substrate_hash01(cell, salt ^ 0x63d835a7u) * center_span,
    );
    let local = fract(grid) * cell_size;
    return 1.0 - smoothstep(radius - 0.75, radius + 0.75, distance(local, center));
}
```

Replace lines that derive `point_step`, `backing_scale`, `physical_hash_point`, `dither_levels`, and the single-pixel fleck threshold with:

```wgsl
let texture_gate = smoothstep(0.18, 0.82, bed_mix);
let broad_tone_levels = (
    substrate_value_noise(point_y_down, 36.0, 0xc13fa9a9u) - 0.5
) * 14.0 * texture_gate;
let grain_mix = substrate_mark(
    point_y_down,
    10.0,
    1.6,
    0.50,
    0x91e10da5u,
) * 0.22 * texture_gate;
let fleck_mix = substrate_mark(
    point_y_down,
    30.0,
    2.8,
    0.28,
    0xd1b54a35u,
) * 0.48 * texture_gate;

var room = mix(core, rim, radial);
room = mix(room, bed, bed_mix * 0.72);
var room_srgb = linear_to_srgb(room);
room_srgb = clamp(
    room_srgb + vec3<f32>(broad_tone_levels / 255.0),
    vec3<f32>(0.0),
    vec3<f32>(1.0),
);
room_srgb = mix(
    room_srgb,
    linear_to_srgb(fleck),
    clamp(grain_mix + fleck_mix, 0.0, 0.60),
);
let straight = vec4<f32>(srgb_to_linear(room_srgb), 1.0);
```

Use absolute `point_y_down`; do not divide texture coordinates by viewport dimensions and do not use `fwidth` inside any substrate helper.

- [ ] **Step 5: Replace the high-frequency-only native readback metric**

Generalize the helper signature and request construction:

```rust
#[cfg(target_os = "macos")]
fn room_only_offscreen(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    backing_scale: f64,
) -> [SceneRenderOutcome; 2] {
    let cpu = compile_fixture(&canonical_materialization_fixture());
    let atlas = full_hud_atlas_for('^', cpu.generation_key.resources, None, None);
    let upload = prepare_scene_upload(&cpu, &atlas).unwrap();
    let shared = SceneGpuShared::create(device, upload.generation_key.device).unwrap();
    let mut candidate =
        materialize_gpu_candidate(device, queue, &shared, &upload, &atlas).unwrap();
    for draw in candidate
        .draw_plan
        .world_blended_unsorted
        .iter_mut()
        .chain(candidate.draw_plan.chrome.prefix.iter_mut())
        .chain(candidate.draw_plan.chrome.suffix.iter_mut())
    {
        draw.instance_range = 0..0;
    }
    let hud = super::super::hud::CaptureSafePreparedHudFrame::zeroed_for_test(
        upload.generation_key.resources,
    );
    let request = render_request_fixture(
        candidate.generation_key,
        candidate.source_revisions,
        candidate.logical_viewport_points,
        backing_scale,
    );
    let mut renderer = SceneRenderer::new(device, queue, &shared);
    std::array::from_fn(|_| {
        renderer
            .render_offscreen(
                device,
                queue,
                &shared,
                &mut candidate,
                request.clone(),
                &hud,
            )
            .expect("isolated retained room renders")
    })
}
```

Keep repeated renders at the same scale byte-identical.

Add this test helper in `render.rs`:

```rust
#[cfg(target_os = "macos")]
fn downsample_rgba(rgba: &[u8], width: usize, block: usize) -> (Vec<u8>, usize) {
    let height = rgba.len() / 4 / width;
    assert_eq!(width % block, 0);
    assert_eq!(height % block, 0);
    let output_width = width / block;
    let output_height = height / block;
    let mut output = Vec::with_capacity(output_width * output_height * 4);
    for output_y in 0..output_height {
        for output_x in 0..output_width {
            for channel in 0..4 {
                let mut sum = 0_u32;
                for y in 0..block {
                    for x in 0..block {
                        let source_x = output_x * block + x;
                        let source_y = output_y * block + y;
                        sum += u32::from(rgba[(source_y * width + source_x) * 4 + channel]);
                    }
                }
                output.push((sum / u32::try_from(block * block).unwrap()) as u8);
            }
        }
    }
    (output, output_width)
}
```

Replace `retained_bed_lower_roi_has_stable_texture_variance` with `retained_bed_lower_roi_has_structured_logical_texture`:

```rust
#[cfg(target_os = "macos")]
#[test]
fn retained_bed_lower_roi_has_structured_logical_texture() {
    let (device, queue) = native_device();
    let [first, repeated] = room_only_offscreen(&device, &queue, 1.0);
    assert_eq!(first.rgba, repeated.rgba, "bed texture must be byte-stable");

    let lower = rgba_roi(&first, [100.0, 285.0, 160.0, 48.0], 1.0);
    let upper = rgba_roi(&first, [100.0, 120.0, 160.0, 96.0], 1.0);
    let (lower_coarse, lower_width) = downsample_rgba(&lower, 160, 4);
    let (upper_coarse, upper_width) = downsample_rgba(&upper, 160, 4);
    let structured = local_trend_residual_variance(&lower_coarse, lower_width);
    let smooth = local_trend_residual_variance(&upper_coarse, upper_width);
    assert!(
        structured > 0.25 && structured > smooth * 2.0,
        "lower bed lacks coherent texture: structured={structured}, smooth={smooth}",
    );
}
```

Update `retained_bed_upper_roi_has_no_substrate_flecks` to call `room_only_offscreen(&device, &queue, 1.0)` and keep its upper-room threshold. Run the native test before changing WGSL and record the measured RED value; the current isolated pixel noise should average away below the new structured threshold.

- [ ] **Step 6: Run GREEN checks and commit Task 3**

Run:

```bash
cargo test --lib --features retained-renderer presentation::companion_effects::tests::bed_texture_
cargo test --lib --features retained-renderer retained_bed_
```

Expected: all CPU and native Metal behavior tests PASS; repeated 1x renders are byte-identical, logical CPU samples are identical at 1x/2x, coherent lower-bed texture survives 4x4 averaging, and the upper room remains smooth.

Commit:

```bash
git add src/presentation/companion_effects.rs src/companion/retained/scene.wgsl src/companion/retained/render.rs
git commit -m "feat(companion): texture the retained substrate"
```

---

### Task 4: Run full verification and launch the optimized companion

**Files:**
- Verify: all files changed in Tasks 1-3
- Generate ignored review bundle: `target/glorp-preview-habitat-grounding/`
- Build and launch ignored app bundle: `target/macos/Glorp.app`

**Interfaces:**
- Consumes: the three committed task deliverables.
- Produces: clean repository checks, deterministic preview artifacts, and a fresh optimized companion for Drew's manual display/resize review.

- [ ] **Step 1: Run formatting, lint, and complete automated coverage**

Run:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --lib --all-features
cargo test --test retained_scene
cargo test --test round_scene
cargo test --features dev-preview --test dev_preview
```

Expected: every command exits `0`; do not ignore warnings or unrelated failures.

- [ ] **Step 2: Generate the deterministic round review bundle**

Run:

```bash
cargo run -- dev-preview --scenario round --out target/glorp-preview-habitat-grounding
```

Expected: `target/glorp-preview-habitat-grounding/manifest.json`, `index.html`, round frame captures, and typed round artifacts are produced without modifying tracked files.

- [ ] **Step 3: Inspect the generated contract before live launch**

Run:

```bash
test -f target/glorp-preview-habitat-grounding/manifest.json
test -f target/glorp-preview-habitat-grounding/index.html
git status --short
```

Expected: both file checks succeed and `git status --short` is empty. If a tracked file changed, inspect it and commit only intentional task-scope changes; do not use `git add -A`.

- [ ] **Step 4: Build and launch a fresh optimized retained companion**

Run:

```bash
cargo xtask companion fresh
```

Expected: the optimized app bundle builds, the previous Glorp companion quits, and `target/macos/Glorp.app` opens. Do not automate fullscreen or move the window between displays.

- [ ] **Step 5: Hand off manual visual checks to Drew**

Ask Drew to verify on the normal and Napster displays:

1. Moss and reeds remain fully inside the circle while resizing and fullscreening.
2. Reeds read behind near-floor moss and behind the pet.
3. The vine stem meets the top circular boundary at every tested size.
4. The ground shows quiet granular variation and clustered flecks without competing with HUD text or the pet silhouette.
5. Animation, resize, fullscreen, and display movement remain crash-free.

If visual tuning is required, change only the approved logical texture constants or candidate inset values, rerun the focused tests plus Step 1, and commit the tuning separately with a descriptive message.
