use glorp::game::evolution::Stage;
use glorp::game::habitat::HabitatPetLayer;
use glorp::game::metabolism::Mood;
use glorp::pet::generation::Species;
use glorp::round::model::{
    derive_round_scene_model, RoundActivityPulse, RoundHelperHealth, RoundPetModel,
    RoundSourceDiversity, RoundVitalBucket,
};
use glorp::storage::state::HabitatPropId;
use glorp::tui::identity::SourceDiversity;
use glorp::tui::view_model::{EventView, SourceStatus, WatchViewModel};
use time::macros::datetime;

#[test]
fn round_scene_excludes_watch_dashboard_and_private_fields() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.helper_status = "provider poll failed: /Users/drew/private/project".into();
    vm.errors = vec!["secret prompt response tool payload /tmp/private.rs".into()];
    vm.recent_events = vec![EventView {
        timestamp: "13:40".into(),
        kind: glorp::tui::style::LogKind::Usage,
        text: "opened /Users/drew/private/project/main.rs".into(),
    }];
    vm.source_breakdown[0].display_name = "client-secret-project".into();
    vm.today_effective_tokens = 123_456.0;
    vm.progress.rate_per_hour = 99_999.0;

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));

    // Direct backstops: prove sensitive WatchViewModel fields were not copied.
    assert!(
        matches!(
            scene.halo.vitals.fed,
            RoundVitalBucket::Low | RoundVitalBucket::Medium | RoundVitalBucket::High
        ),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        matches!(
            scene.halo.vitals.happiness,
            RoundVitalBucket::Low | RoundVitalBucket::Medium | RoundVitalBucket::High
        ),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        matches!(
            scene.halo.vitals.energy,
            RoundVitalBucket::Low | RoundVitalBucket::Medium | RoundVitalBucket::High
        ),
        "private data leaked into RoundSceneModel"
    );
    let _: &[HabitatPropId] = &scene.room.prop_landmarks;
    assert!(
        scene.moments.is_empty(),
        "private data leaked into RoundSceneModel"
    );
    assert_eq!(
        scene.pet,
        RoundPetModel {
            seed: "fixture-seed".into(),
            species: Species::Fuzz,
            stage: Stage::S0,
            mood: Mood::Content,
            art_lines: vm.pet_art.clone(),
            art_spans: vm.pet_spans.clone(),
            asleep: false,
            breath_offset_y: 0,
            facing: 1,
        },
        "private data leaked into RoundSceneModel"
    );

    let debug = format!("{scene:?}");

    assert!(
        !debug.contains("secret"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("/Users/drew"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("prompt"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("response"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("tool payload"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("123456"),
        "private data leaked into RoundSceneModel"
    );
    assert!(
        !debug.contains("99999"),
        "private data leaked into RoundSceneModel"
    );
}

#[test]
fn round_scene_model_does_not_carry_companion_hud_metrics() {
    let now = datetime!(2026 - 07 - 06 20:00 UTC);
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.today_effective_tokens = 842_000_000.0;
    vm.rate_momentum.pulse.current_tokens = 31_000_000.0;
    vm.daily_comparison = glorp::tui::view_model::DailyComparison {
        today_provider_day: time::macros::date!(2026 - 07 - 06),
        yesterday_provider_day: time::macros::date!(2026 - 07 - 05),
        today_tokens: 842_000_000.0,
        yesterday_tokens: Some(678_000_000.0),
        today_snapshot_state: glorp::usage::snapshot::SnapshotState::Current,
        yesterday_snapshot_state: glorp::usage::snapshot::SnapshotState::Current,
        today_observed_at: Some(now),
        yesterday_observed_at: Some(now - time::Duration::days(1)),
        unavailable_reason: None,
        fraction_of_yesterday: Some(842_000_000.0 / 678_000_000.0),
    };

    let scene = glorp::round::model::derive_round_scene_model(&vm, now);
    let debug = format!("{scene:#?}");

    for forbidden in [
        "daily_comparison",
        "fraction_of_yesterday",
        "842000000",
        "31000000",
        "124% yday",
        "/10m",
    ] {
        assert!(
            !debug.contains(forbidden),
            "RoundSceneModel leaked companion HUD metric {forbidden}: {debug}"
        );
    }
}

#[test]
fn round_scene_maps_required_v1_signals() {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.activity_identity.source_diversity = SourceDiversity::DualLane;
    vm.last_feed_pulse_at = Some(datetime!(2026-06-13 17:59:59 UTC));
    let codex = vm
        .source_health
        .iter_mut()
        .find(|s| s.name == "codex")
        .expect("codex source health entry");
    codex.status = SourceStatus::Diagnostic;
    codex.diagnostic_code = Some("missing_helper".into());
    codex.diagnostic_message = Some("private helper path".into());

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 18:00 UTC));

    assert_eq!(scene.pet.seed, "fixture-seed", "expected fixture seed");
    assert_eq!(
        scene.pet.species, vm.pet_render.generated_species,
        "expected pet species"
    );
    assert_eq!(scene.pet.stage, vm.pet_render.stage, "expected pet stage");
    assert_eq!(
        scene.room.prop_landmarks.len(),
        2,
        "expected two prop landmarks"
    );
    assert_eq!(
        scene.halo.source_diversity,
        RoundSourceDiversity::Dual,
        "expected Dual source diversity"
    );
    assert_eq!(
        scene.halo.helper_health,
        RoundHelperHealth::Trouble,
        "expected helper health trouble"
    );
    assert!(
        matches!(scene.halo.activity_pulse, RoundActivityPulse::Recent { .. }),
        "expected Recent activity pulse"
    );
}

#[test]
fn round_scene_uses_night_calm_for_asleep_state() {
    let mut vm = WatchViewModel::fixture();
    vm.day_context.asleep = true;
    vm.life_profile.calm_mode = true;

    let scene = derive_round_scene_model(&vm, datetime!(2026-06-13 08:00 UTC));

    assert!(scene.lifecycle.asleep);
    assert!(scene.lifecycle.calm);
    assert!(scene.halo.activity_pulse.is_quiet());
}

#[test]
fn round_scene_contract_has_no_mood_aura_alias_or_serialized_json() {
    use glorp::presentation::companion_scene::scene::build_scene_generation;
    use glorp::presentation::companion_scene::{
        CompanionLogicalLayout, CompanionProjectionClock, CompanionSceneProjectionInput,
        CompanionSceneSnapshot, DeviceEpoch, LayoutGeneration, ResourceGeneration,
        SceneGenerationKey,
    };

    const COLUMNS: u16 = 44;
    const ROWS: u16 = 18;
    let now = datetime!(2026-07-08 18:00 UTC);
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    let rendered = glorp::pet::render::render_pet(
        &glorp::pet::generation::generate_pet(&vm.pet_render.seed)
            .with_species(vm.pet_render.generated_species),
        vm.pet_render.stage,
        vm.pet_render.mood,
        glorp::pet::render::AnimationFrame::default(),
    );
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    let snapshot = CompanionSceneSnapshot::project_with_input(
        &vm,
        CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(now, 0),
            CompanionLogicalLayout::round(360.0, 360.0),
            COLUMNS,
            ROWS,
            glorp::round::scene::current_round_motion_clearance(ROWS),
        ),
    )
    .expect("fixture projects");
    let generation = build_scene_generation(
        &snapshot,
        SceneGenerationKey {
            device: DeviceEpoch(1),
            layout: LayoutGeneration(1),
            resources: ResourceGeneration(1),
        },
    )
    .expect("fixture builds");
    let aliases = generation
        .template()
        .primitives
        .iter()
        .map(|primitive| {
            generation
                .template()
                .nodes
                .iter()
                .find(|node| node.id == primitive.node)
                .expect("primitive owns a declared node")
                .alias
                .as_str()
        })
        .collect::<Vec<_>>();
    let serialized = serde_json::to_string(&aliases).expect("primitive aliases serialize");

    assert!(!aliases.contains(&"pet.aura.mood"));
    assert!(!serialized.contains("pet.aura.mood"));
}

#[test]
fn round_scene_tank_life_foreground_avoids_pet_face_and_bottom_hud() {
    let now = time::macros::datetime!(2026-07-08 18:00 UTC);
    let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(60, now.date());
    vm.habitat.tank_life_local_date = time::macros::date!(2026 - 07 - 08);
    vm.habitat.tank_life_calendar_age_days = 60;

    let scene = glorp::round::scene::build_round_scene_draw_list(
        &vm,
        now,
        44,
        18,
        &glorp::round::scene::companion_roam_motion(),
    );

    let protected =
        glorp::round::scene::round_tank_life_protected_regions_for_test(scene.pet_rect, 44, 18);
    let geometry = glorp::round::scene::round_tank_life_geometry(44, 18);
    let canonical = glorp::tui::component::canonical_daily_cast(
        &vm.habitat.earned_inhabitants,
        &vm.pet_render.seed,
        vm.habitat.tank_life_local_date,
        vm.habitat.tank_life_calendar_age_days,
    );
    let projected = glorp::tui::component::project_tank_life_cast(&canonical, &geometry);
    let placements = glorp::tui::component::tank_life_placements_for(
        &glorp::tui::component::TankLifeRenderInput {
            rendered_ids: projected.rendered_ids,
            pet_seed: &vm.pet_render.seed,
            local_date: vm.habitat.tank_life_local_date,
            now,
            geometry: &geometry,
            pet_protected_regions: &protected.pet_face,
            color_capability: glorp::tui::style::ColorCapability::Truecolor,
            life_profile: vm.life_profile.clone(),
            asleep: vm.day_context.asleep,
        },
    );

    for cell in placements.iter().flat_map(|placement| &placement.cells) {
        assert!(
            !protected
                .bottom_hud
                .iter()
                .any(|region| glorp::tui::component::rect_contains(*region, cell.col, cell.row)),
            "tank life cells must stay clear of bottom HUD reserve; got {:?} at ({}, {})",
            cell.glyph,
            cell.col,
            cell.row
        );
        if cell.pet_layer == HabitatPetLayer::Foreground {
            assert!(
                !protected
                    .pet_face
                    .iter()
                    .any(|region| glorp::tui::component::rect_contains(*region, cell.col, cell.row)),
                "foreground tank life cells must stay clear of protected pet face; got {:?} at ({}, {})",
                cell.glyph,
                cell.col,
                cell.row
            );
        }
    }
}

#[test]
fn purposeful_motion_round_paths_share_locomotion_projection_contract() {
    use glorp::presentation::companion_scene::{
        CompanionLogicalLayout, CompanionProjectionClock, CompanionSceneProjectionInput,
        CompanionSceneSnapshot,
    };
    use glorp::round::depth::resolve_smooth_depth;
    use glorp::round::placement::resolve_round_depth_placement;

    const COLS: u16 = 44;
    const ROWS: u16 = 18;
    const EXTENT_POINTS: f32 = 360.0;

    let now = datetime!(2026-07-17 12:34:56 UTC);
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    let rendered = glorp::pet::render::render_pet(
        &glorp::pet::generation::generate_pet(&vm.pet_render.seed)
            .with_species(vm.pet_render.generated_species),
        vm.pet_render.stage,
        vm.pet_render.mood,
        glorp::pet::render::AnimationFrame::default(),
    );
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    vm.day_context.wake_resume = Some(glorp::tui::day::WakeResume {
        from_eval_utc: now - time::Duration::seconds(16),
        woke_at_utc: now - time::Duration::seconds(4),
    });
    let motion = glorp::round::scene::companion_roam_motion();
    let draw_list = glorp::round::scene::build_round_scene_draw_list(&vm, now, COLS, ROWS, &motion);
    let draw_placement =
        glorp::round::scene::companion_pet_placement(&vm, now, COLS, ROWS, &motion);
    let snapshot = CompanionSceneSnapshot::project_with_input(
        &vm,
        CompanionSceneProjectionInput::round(
            CompanionProjectionClock::new(now, 0),
            CompanionLogicalLayout::round(EXTENT_POINTS, EXTENT_POINTS),
            COLS,
            ROWS,
            glorp::round::scene::current_round_motion_clearance(ROWS),
        ),
    )
    .expect("valid companion scene projection");

    let projection = draw_placement.motion_projection();
    let resolved_depth = resolve_smooth_depth(
        projection.normalized_depth,
        glorp::round::depth::depth_lifecycle_scale(false, false),
    )
    .expect("locomotion depth is normalized");
    let expected_depth_placement = resolve_round_depth_placement(
        projection,
        resolved_depth,
        glorp::round::motion::RoundCompanionMotionViewport {
            grid_columns: COLS,
            grid_rows: ROWS,
            width_points: EXTENT_POINTS,
            height_points: EXTENT_POINTS,
            clearance: glorp::round::scene::current_round_motion_clearance(ROWS),
        },
    )
    .expect("shared projection stays inside the aperture");

    assert_eq!(draw_list.pet_rect, draw_placement.classic_rect);
    assert_eq!(snapshot.frame.pet_depth, projection.normalized_depth);
    assert_eq!(snapshot.frame.facing, projection.facing);
    assert_eq!(
        draw_placement.fractional_motion_origin_top_left.x,
        projection.motion_origin_top_left_cells.x
    );
    assert_eq!(
        draw_placement.fractional_motion_origin_top_left.y,
        projection.motion_origin_top_left_cells.y
    );
    assert_eq!(
        snapshot.frame.pet_anchor_points,
        expected_depth_placement.anchor_top_left_points
    );
}

#[test]
fn tank_routes_avoid_composition_chrome_and_foreground_props() {
    use glorp::game::habitat::{HabitatPropKind, HEAVY_SESSION_PLANTER};
    use glorp::presentation::companion_scene::{
        AuthoredDepthSnapshot, CompanionLogicalLayout, CompanionProjectionClock,
        CompanionSceneProjectionInput, CompanionSceneSnapshot, TankLayerSnapshot,
    };
    use glorp::storage::state::HabitatPropSource;
    use glorp::tui::view_model::EarnedHabitatPropView;

    const COLS: u16 = 44;
    const ROWS: u16 = 18;
    const EXTENT_POINTS: f32 = 260.0;

    let now = datetime!(2026-07-08 18:00 UTC);
    let mut vm = WatchViewModel::fixture_with_tank_inhabitants_for_age(60, now.date());
    vm.habitat.earned_props.push(EarnedHabitatPropView {
        id: HabitatPropId::new(HEAVY_SESSION_PLANTER),
        earned_at: time::OffsetDateTime::UNIX_EPOCH,
        kind: HabitatPropKind::Trophy,
        display_priority: 148,
        source: HabitatPropSource::HeavySession,
    });
    let rendered = glorp::pet::render::render_pet(
        &glorp::pet::generation::generate_pet(&vm.pet_render.seed)
            .with_species(vm.pet_render.generated_species),
        vm.pet_render.stage,
        Mood::Content,
        glorp::pet::render::AnimationFrame::default(),
    );
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;

    let clearance = glorp::round::scene::current_round_motion_clearance(ROWS);
    let input = CompanionSceneProjectionInput::round(
        CompanionProjectionClock::new(now, 0),
        CompanionLogicalLayout::round(EXTENT_POINTS, EXTENT_POINTS),
        COLS,
        ROWS,
        clearance,
    );
    let snapshot = CompanionSceneSnapshot::project_with_input(&vm, input).unwrap();
    let scene = glorp::round::scene::build_round_scene_draw_list(
        &vm,
        now,
        COLS,
        ROWS,
        &glorp::round::scene::companion_roam_motion(),
    );
    let protected =
        glorp::round::scene::round_tank_life_protected_regions_for_test(scene.pet_rect, COLS, ROWS);

    // Mirror the public, fixed gauge geometry contract to recover the innermost
    // safe radii used by CompanionComposition, then apply its conservative floor.
    let aperture_radius = f64::from(EXTENT_POINTS) / 2.0;
    let outer_inset = 3.0_f64.max(aperture_radius * 0.012);
    let xp_width = (aperture_radius * 0.050).clamp(6.0, 16.0);
    let daily_width = (aperture_radius * 0.040).clamp(5.0, 13.0);
    let pace_width = (aperture_radius * 0.034).clamp(4.0, 11.0);
    let lane_gap = (aperture_radius * 0.010).clamp(1.5, 4.0);
    let xp_radius = aperture_radius - outer_inset - xp_width / 2.0;
    let daily_radius = xp_radius - xp_width / 2.0 - lane_gap - daily_width / 2.0;
    let pace_radius = daily_radius - daily_width / 2.0 - lane_gap - pace_width / 2.0;
    let inner_radius_points = pace_radius - pace_width / 2.0;
    let radius_cols = ((inner_radius_points / (f64::from(EXTENT_POINTS) / f64::from(COLS))) - 0.5)
        .max(0.0)
        .floor() as i32;
    let radius_rows = ((inner_radius_points / (f64::from(EXTENT_POINTS) / f64::from(ROWS))) - 0.5)
        .max(0.0)
        .floor() as i32;

    let cell_extent = [
        EXTENT_POINTS / f32::from(COLS),
        EXTENT_POINTS / f32::from(ROWS),
    ];
    let foreground_prop_rects = snapshot
        .topology
        .visible_props
        .iter()
        .filter(|prop| prop.authored_depth == AuthoredDepthSnapshot::Foreground)
        .filter_map(|prop| {
            let frame = snapshot
                .frame
                .prop_instances
                .iter()
                .find(|frame| frame.slot == prop.stable_order)?;
            frame.visible.then(|| {
                let x = (frame.origin_points[0] / cell_extent[0]).round() as i32;
                let y = (frame.origin_points[1] / cell_extent[1]).round() as i32;
                let width = (frame.footprint_points[0] / cell_extent[0]).round() as i32;
                let height = (frame.footprint_points[1] / cell_extent[1]).round() as i32;
                [x - 1, y - 1, x + width + 1, y + height + 1]
            })
        })
        .collect::<Vec<_>>();
    assert!(
        !foreground_prop_rects.is_empty(),
        "fixture must project an accepted foreground prop"
    );

    let hud = [
        ((f32::from(COLS) / 2.0) - (f32::from(COLS) * 0.58) / 2.0).floor() as i32,
        (f32::from(ROWS) * 0.58).floor() as i32,
        ((f32::from(COLS) / 2.0) + (f32::from(COLS) * 0.58) / 2.0).ceil() as i32,
        (f32::from(ROWS) * 0.90).ceil() as i32,
    ];
    let bottom = [
        0,
        i32::from(ROWS - clearance.bottom_reserved_rows),
        i32::from(COLS),
        i32::from(ROWS),
    ];
    let contains = |rect: [i32; 4], col: u16, row: u16| {
        i32::from(col) >= rect[0]
            && i32::from(col) < rect[2]
            && i32::from(row) >= rect[1]
            && i32::from(row) < rect[3]
    };

    let cells = snapshot
        .content
        .tank_animation_states
        .iter()
        .flat_map(|tank| &tank.cells)
        .collect::<Vec<_>>();
    assert!(!cells.is_empty(), "fixture must resolve visible tank cells");
    for cell in cells {
        let dx = i32::from(cell.col) - i32::from(COLS / 2);
        let dy = i32::from(cell.row) - i32::from(ROWS / 2);
        assert!(
            dx * dx * radius_rows * radius_rows + dy * dy * radius_cols * radius_cols
                <= radius_cols * radius_cols * radius_rows * radius_rows,
            "tank cell {:?} at ({}, {}) escaped the gauge-safe aperture",
            cell.glyph,
            cell.col,
            cell.row
        );
        assert!(!contains(hud, cell.col, cell.row));
        assert!(!contains(bottom, cell.col, cell.row));

        if cell.layer == TankLayerSnapshot::Foreground {
            assert!(
                !protected.pet_face.iter().any(|region| {
                    glorp::tui::component::rect_contains(*region, cell.col, cell.row)
                }),
                "foreground tank cell at ({}, {}) overlaps the pet face",
                cell.col,
                cell.row
            );
            assert!(
                foreground_prop_rects
                    .iter()
                    .all(|rect| !contains(*rect, cell.col, cell.row)),
                "foreground tank cell at ({}, {}) overlaps a foreground prop",
                cell.col,
                cell.row
            );
        }
    }
}

/// The round companion renders `WatchViewModel.pet_art` into a circular
/// aperture; a Heavy-tier Glitch S6 day earns up to three persistent repair
/// marks (one-cell `Pattern`/`Accent` spans carrying a `+ = : .` glyph — see
/// `glitch_repair_marks_use_soft_roles_not_corruption` in `src/pet/render.rs`
/// for the same identification method). This proves at least one of those
/// marks actually lands on a visible (non-masked) cell once the pet art is
/// positioned in the round scene's grid, not clipped by the circle.
#[cfg(feature = "dev-preview")]
#[test]
fn round_glitch_s6_keeps_a_repair_mark_inside_aperture() {
    use glorp::pet::render::PaletteRoleName;
    use glorp::round::layout::RoundAperture;
    use glorp::round::preview::render_round_preview_frame_from_vm;
    use glorp::round::scene::{build_round_scene_draw_list, CompanionMotion};

    const WIDTH: u16 = 52;
    const HEIGHT: u16 = 52;
    let now = datetime!(2026-06-13 18:00 UTC);

    // Mirror the `round-glitch-patched-s6` preview fixture
    // (src/dev_preview/round.rs): a calm S6 Glitch on a Heavy-tier day
    // (today_ratio 1.7, date_seed 42) earns 3 repair marks.
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.pet_render.seed = "glorp-preview-glitch-persistence".to_string();
    vm.pet_render.generated_species = Species::Glitch;
    vm.pet_render.stage = Stage::S6;
    vm.day_context.date_seed = 42;
    vm.day_context.today_ratio = 1.7;
    vm.life_profile.burst_level = 0.0;
    vm.life_profile.calm_mode = true;
    glorp::commands::watch::rerender_pet_for_view_model(
        &mut vm,
        now.unix_timestamp().max(0) as u64,
        false,
        now,
    )
    .expect("fixture should rerender");

    // Find a declared repair mark: a one-cell Pattern/Accent span carrying one
    // of the soldered repair glyphs.
    let repair_span = vm
        .pet_spans
        .iter()
        .find(|span| {
            matches!(
                span.role,
                PaletteRoleName::Pattern | PaletteRoleName::Accent
            ) && span.end == span.start + 1
        })
        .expect("Heavy-tier Glitch S6 fixture should carry at least one repair mark span");
    let mark_row = repair_span.line;
    let mark_col = repair_span.start;
    let mark_glyph = vm.pet_art[mark_row]
        .chars()
        .nth(mark_col)
        .expect("repair mark column should be in-bounds");
    assert!(
        "+=:.".contains(mark_glyph),
        "repair mark glyph should be one of the soldered repair glyphs, got {mark_glyph:?}"
    );

    // Map the mark's art-local (row, col) to the round scene's absolute grid
    // position for this vm/now, using the same seam and default companion
    // motion the round-glitch-patched-s6 preview renders with. The seam may
    // horizontally mirror the art (facing left) before placing it at
    // `pet_rect`, which — per `mirror_spans` in
    // `src/tui/panels/pet/art_lines.rs` — maps a one-cell span's column `c` to
    // `line_len - 1 - c` (none of the repair glyphs are direction-swapped by
    // `mirror_char`, so the glyph itself is unaffected). Check both the
    // unmirrored and mirrored column so the assertion holds regardless of
    // which way the pet is facing at this deterministic `now`.
    let companion_scene =
        build_round_scene_draw_list(&vm, now, WIDTH, HEIGHT, &CompanionMotion::default());
    let line_len = vm.pet_art[mark_row].chars().count();
    let mirrored_col = line_len - 1 - mark_col;
    let candidate_cols = [mark_col, mirrored_col];

    let aperture = RoundAperture::new(WIDTH, HEIGHT);
    let frame = render_round_preview_frame_from_vm(
        "round-glitch-s6-repair-test",
        "Round Glitch S6 Repair Test",
        &vm,
        now,
        WIDTH,
        HEIGHT,
        glorp::round::layout::RoundRenderCapabilities::preview_truecolor(),
    );

    let visible_match = candidate_cols.iter().any(|&col| {
        let abs_x = companion_scene.pet_rect.x + col as u16;
        let abs_y = companion_scene.pet_rect.y + mark_row as u16;
        if !aperture.contains(abs_x as f32, abs_y as f32) {
            return false;
        }
        frame.cells.iter().any(|c| {
            c.x == abs_x
                && c.y == abs_y
                && !c.outside_aperture
                && c.symbol == mark_glyph.to_string()
        })
    });

    assert!(
        visible_match,
        "expected the repair mark glyph {mark_glyph:?} to land on a visible \
         (non-masked) cell at row {mark_row}, cols {candidate_cols:?} \
         (pet_rect={:?}); it was clipped or overwritten",
        companion_scene.pet_rect
    );
}
