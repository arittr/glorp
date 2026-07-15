#![cfg(all(target_os = "macos", feature = "retained-renderer"))]

const APP_SOURCE: &str = include_str!("../src/companion/app.rs");
const RETAINED_SOURCE: &str = include_str!("../src/companion/retained.rs");
const HOST_SOURCE: &str = include_str!("../src/companion/retained/host.rs");

fn source_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_index = source
        .find(start)
        .unwrap_or_else(|| panic!("missing source marker {start}"));
    let tail = &source[start_index..];
    let end_offset = tail
        .find(end)
        .unwrap_or_else(|| panic!("missing source marker {end}"));
    &tail[..end_offset]
}

#[test]
fn rollout_shadow_services_but_never_activates_or_presents_scene_output() {
    let shadow = source_between(
        APP_SOURCE,
        "\nfn service_shadow_scene_runtime(",
        "\nfn service_live_scene_runtime(",
    );
    assert!(shadow.contains("advance_scene_generation(false)"));
    for forbidden in [
        "materialize_scene_candidate",
        "activate_candidate(",
        "present_active_scene(",
        "queue.present",
    ] {
        assert!(
            !shadow.contains(forbidden),
            "shadow must not call {forbidden}"
        );
    }
}

#[test]
fn shadow_install_and_service_create_no_scene_gpu_objects() {
    let install = source_between(
        HOST_SOURCE,
        "\n    pub(in crate::companion) fn install_scene_runtime(",
        "\n    pub(in crate::companion) fn has_scene_runtime(",
    );
    for forbidden in [
        "SceneGpuShared::create(",
        "SceneRenderer::new(",
        "ensure_scene_gpu_state(",
        "materialize_scene_candidate(",
    ] {
        assert!(
            !install.contains(forbidden),
            "CPU-only scene install must not call {forbidden}"
        );
    }

    let shadow = source_between(
        APP_SOURCE,
        "\nfn service_shadow_scene_runtime(",
        "\nfn service_live_scene_runtime(",
    );
    assert!(shadow.contains("advance_scene_generation(false)"));
    assert!(!shadow.contains("ensure_scene_gpu_state("));
}

#[test]
fn rollout_live_activates_then_presents_active_scene_versions() {
    let live = source_between(
        APP_SOURCE,
        "\nfn service_live_scene_runtime(",
        "\nfn handle_scene_runtime_failure(",
    );
    assert!(live.contains("advance_scene_generation(true)"));
    assert!(live.contains("activate_candidate("));
    assert!(live.contains("present_active_scene("));
    assert!(HOST_SOURCE.contains("Result<ScenePresentOutcome, RetainedFailureCategory>"));
    assert!(HOST_SOURCE.contains("ScenePresentOutcome::Presented(version)"));
}

#[test]
fn terminal_failure_retains_the_host_and_never_constructs_smooth_fallback() {
    let freeze = source_between(
        APP_SOURCE,
        "\nfn freeze_retained(",
        "\nfn record_frame_preparation_error(",
    );
    assert!(freeze.contains("record_terminal_failure(error)"));
    for forbidden in [
        "request_fallback",
        "restore_appkit",
        "new_round_view",
        "acknowledge_smooth_paint",
        "retained_host.take()",
        "record_fallback",
    ] {
        assert!(
            !freeze.contains(forbidden),
            "terminal failure path must not call {forbidden}"
        );
    }
}

#[test]
fn hidden_scene_path_only_marks_hidden_and_reveal_coalesces_once() {
    let hidden = source_between(
        HOST_SOURCE,
        "\n    pub(in crate::companion) fn hide_scene_runtime(",
        "\n    pub(in crate::companion) fn reveal_scene_runtime(",
    );
    assert_eq!(hidden.matches(".set_hidden() ").count(), 0);
    assert_eq!(hidden.matches(".set_hidden();").count(), 1);
    for forbidden in [
        "prepare_snapshot(",
        "write_buffer",
        "get_current_texture",
        "create_command_encoder",
        "queue.submit",
        "queue.present",
    ] {
        assert!(
            !hidden.contains(forbidden),
            "hidden path must not call {forbidden}"
        );
    }

    let reveal = source_between(
        HOST_SOURCE,
        "\n    pub(in crate::companion) fn reveal_scene_runtime(",
        "\n    pub(in crate::companion) fn retry_scene_replacement(",
    );
    assert_eq!(reveal.matches(".coalesce_hidden_snapshot(").count(), 1);

    let app_reveal = source_between(
        APP_SOURCE,
        "\nfn reveal_scene_runtime(",
        "\nfn prepare_scene_runtime_tick(",
    );
    assert_eq!(app_reveal.matches("host.reveal_scene_runtime(").count(), 1);
}

#[test]
fn live_surface_delta_route_uses_transactional_retained_delta_machinery() {
    let present = source_between(
        HOST_SOURCE,
        "\n    fn present_active_scene(",
        "\n    fn advance_scene_generation(",
    );
    assert!(present.contains(".submit_active_to_surface("));

    let delta_route = source_between(
        RETAINED_SOURCE,
        "\n    fn submit_active_to_surface(",
        "\n    fn active_version(",
    );
    assert!(delta_route.contains("renderer.submit_active_to_surface_with_delta("));
    assert!(!delta_route.contains("build_scene_generation"));
}

#[test]
fn active_present_skips_a_stale_generation_before_surface_acquire() {
    let present = source_between(
        HOST_SOURCE,
        "\n    fn present_active_scene(",
        "\n    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.",
    );
    let extent_guard = present
        .find("generations.active_surface_extent_matches(")
        .expect("active present guards a resized surface from the stale generation");
    let acquire = present
        .find("self.surface.get_current_texture()")
        .expect("active present acquires a surface texture");

    assert!(extent_guard < acquire);
    assert!(present[extent_guard..acquire]
        .contains("return Ok(ScenePresentOutcome::Skipped(SkipReason::Outdated));"));

    for (start, end, reconcile_call) in [
        (
            "\n    pub(in crate::companion) fn reconcile_scene_snapshot(",
            "\n    pub(in crate::companion) fn reconcile_scene_frame(",
            ".reconcile_snapshot(",
        ),
        (
            "\n    pub(in crate::companion) fn reconcile_scene_frame(",
            "\n    pub(in crate::companion) fn hide_scene_runtime(",
            ".reconcile_frame_projection(",
        ),
    ] {
        let reconcile = source_between(HOST_SOURCE, start, end);
        let resize = reconcile
            .find("self.host.resize_surface_if_needed(view)?")
            .expect("coordinator resize precedes scene reconciliation");
        let rebind = reconcile
            .find("activation.generations.rebind_surface(change.epoch)?")
            .expect("surface change rebinds the scene runtime");
        let reconcile_scene = reconcile
            .find(reconcile_call)
            .expect("surface rebind precedes the applicable scene reconciliation");
        assert!(resize < rebind);
        assert!(rebind < reconcile_scene);
    }
}

#[test]
fn shadow_metrics_bind_the_pending_cpu_scene_version() {
    let metrics = source_between(
        HOST_SOURCE,
        "\n    pub(crate) fn runtime_metrics_snapshot(",
        "\n}\n\nstruct DirectLifetimeAuditExecutor",
    );
    assert!(metrics.contains(".metrics_version()"));
    assert!(metrics.contains("semantic_revision: scene_version"));
    assert!(metrics.contains("frame_revision: scene_version"));
    assert!(RETAINED_SOURCE.contains("fn metrics_version("));
}

#[test]
fn live_retries_an_unpresented_delta_before_reconciling_a_new_snapshot() {
    let live_tick = source_between(
        APP_SOURCE,
        "state.scene_runtime_rollout == SceneRuntimeRollout::Live",
        "state.scene_runtime_rollout == SceneRuntimeRollout::Shadow",
    );
    assert!(live_tick.contains("scene_active_delta_pending()"));
    assert!(live_tick.contains("if !active_delta_pending"));
    assert!(HOST_SOURCE.contains("fn scene_active_delta_pending("));
    assert!(RETAINED_SOURCE.contains("fn active_delta_pending("));
}

#[test]
fn reveal_retries_the_pre_hidden_active_delta_before_committing_latest() {
    let host_reveal = source_between(
        HOST_SOURCE,
        "\n    pub(in crate::companion) fn reveal_scene_runtime(",
        "\n    pub(in crate::companion) fn advance_scene_generation(",
    );
    let pending_check = host_reveal
        .find("active_delta_pending()")
        .expect("reveal must inspect the external-to-logical revision gap");
    let coalesce = host_reveal
        .find("coalesce_hidden_snapshot(")
        .expect("reveal must still reconcile the latest hidden snapshot");
    assert!(pending_check < coalesce);
    assert!(host_reveal.contains("return Ok(false)"));

    let app_reveal = source_between(
        APP_SOURCE,
        "\nfn reveal_scene_runtime(",
        "\nfn prepare_scene_runtime_tick(",
    );
    assert!(app_reveal.contains("state.scene_runtime_hidden = !revealed"));
}

#[test]
fn retained_snapshot_keeps_fixed_prop_slots_when_composition_hides_an_accent() {
    use glorp::game::evolution::Stage;
    use glorp::game::habitat::HABITAT_PROP_CATALOG;
    use glorp::game::metabolism::Mood;
    use glorp::pet::generation::{generate_pet, Species};
    use glorp::pet::render::{render_pet, AnimationFrame};
    use glorp::presentation::companion_scene::{
        AuthoredDepthSnapshot, CompanionLogicalLayout, CompanionProjectionClock,
        CompanionSceneProjectionInput, CompanionSceneSnapshot, PropZoneSnapshot,
    };
    use glorp::tui::view_model::{EarnedHabitatPropView, WatchViewModel};
    use time::macros::datetime;

    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.habitat.earned_props = HABITAT_PROP_CATALOG
        .iter()
        .map(|spec| EarnedHabitatPropView {
            id: glorp::storage::state::HabitatPropId::new(spec.id),
            earned_at: time::OffsetDateTime::UNIX_EPOCH,
            kind: spec.kind,
            display_priority: spec.display_priority,
            source: glorp::storage::state::HabitatPropSource::LifetimeTokens {
                threshold: spec.lifetime_threshold.unwrap_or(0.0),
            },
        })
        .collect();
    let rendered = render_pet(
        &generate_pet("retained-composition-hidden-accent").with_species(Species::Fuzz),
        Stage::S3,
        Mood::Content,
        AnimationFrame::default(),
    );
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;
    vm.pet_art = rendered.lines;
    vm.pet_spans = rendered.spans;
    let input = CompanionSceneProjectionInput::round(
        CompanionProjectionClock::new(datetime!(2026-07-15 12:00 UTC), 0),
        CompanionLogicalLayout::round(260.0, 260.0),
        44,
        18,
        glorp::round::scene::current_round_motion_clearance(18),
    );

    let first = CompanionSceneSnapshot::project_with_input(&vm, input).unwrap();
    let second = CompanionSceneSnapshot::project_with_input(&vm, input).unwrap();
    assert_eq!(first.topology.visible_props.len(), 10);
    assert_eq!(
        first.topology.visible_props.len(),
        first.content.prop_animation_states.len()
    );
    assert_eq!(
        first.topology.visible_props.len(),
        first.frame.prop_instances.len()
    );
    assert_eq!(
        first.topology.visible_props.len(),
        second.topology.visible_props.len()
    );
    assert_eq!(
        first.content.prop_animation_states.len(),
        second.content.prop_animation_states.len()
    );
    let lowest_priority_slot = first.topology.visible_props.last().unwrap().stable_order;
    let first_frame = first
        .frame
        .prop_instances
        .iter()
        .find(|frame| frame.slot == lowest_priority_slot)
        .unwrap();
    let second_frame = second
        .frame
        .prop_instances
        .iter()
        .find(|frame| frame.slot == lowest_priority_slot)
        .unwrap();
    assert_eq!(first_frame.opacity, 0.0);
    assert_eq!(second_frame.opacity, 0.0);
    assert!(!first_frame.visible);
    assert!(!second_frame.visible);
    assert_eq!(first_frame.origin_points, second_frame.origin_points);
    assert_eq!(first_frame.footprint_points, second_frame.footprint_points);
    assert!(first_frame
        .footprint_points
        .iter()
        .all(|extent| *extent > 0.0));
    assert_eq!(first_frame.contact_shadow_strength, 0.0);
    assert_eq!(second_frame.contact_shadow_strength, 0.0);

    for prop in &first.topology.visible_props {
        let frame = first
            .frame
            .prop_instances
            .iter()
            .find(|frame| frame.slot == prop.stable_order)
            .unwrap();
        let grounded = matches!(
            prop.zone,
            PropZoneSnapshot::FloorLeft | PropZoneSnapshot::FloorMid | PropZoneSnapshot::FloorRight
        );
        let expected = if frame.visible && grounded {
            match prop.authored_depth {
                AuthoredDepthSnapshot::Background => 0.0,
                AuthoredDepthSnapshot::BehindPet => 0.24,
                AuthoredDepthSnapshot::Foreground => 0.34,
            }
        } else {
            0.0
        };
        assert_eq!(
            frame.contact_shadow_strength, expected,
            "{}",
            prop.catalog_id
        );
    }
}

#[test]
fn retained_full_cast_composition_matrix() {
    use glorp::game::evolution::Stage;
    use glorp::game::habitat::{HabitatPropKind, HABITAT_PROP_CATALOG};
    use glorp::game::metabolism::Mood;
    use glorp::pet::generation::{generate_pet, Species};
    use glorp::pet::render::{render_pet, AnimationFrame};
    use glorp::presentation::companion_scene::{
        CompanionLogicalLayout, CompanionProjectionClock, CompanionSceneProjectionInput,
        CompanionSceneSnapshot,
    };
    use glorp::tui::view_model::{EarnedHabitatPropView, WatchViewModel};
    use time::macros::{date, datetime};

    const COLUMNS: u16 = 44;
    const ROWS: u16 = 18;
    const SURFACES: &[(f32, f32)] = &[
        (260.0, 260.0),
        (360.0, 360.0),
        (480.0, 480.0),
        (720.0, 720.0),
        (480.0, 360.0),
        (360.0, 480.0),
    ];

    fn full_cast_fixture() -> WatchViewModel {
        let mut vm =
            WatchViewModel::fixture_with_tank_inhabitants_for_age(120, date!(2026 - 07 - 15));
        vm.habitat.earned_props = HABITAT_PROP_CATALOG
            .iter()
            .map(|spec| EarnedHabitatPropView {
                id: glorp::storage::state::HabitatPropId::new(spec.id),
                earned_at: time::OffsetDateTime::UNIX_EPOCH,
                kind: spec.kind,
                display_priority: spec.display_priority,
                source: glorp::storage::state::HabitatPropSource::LifetimeTokens {
                    threshold: spec.lifetime_threshold.unwrap_or(0.0),
                },
            })
            .collect();
        let rendered = render_pet(
            &generate_pet("retained-full-cast-composition-matrix").with_species(Species::Fuzz),
            Stage::S3,
            Mood::Content,
            AnimationFrame::default(),
        );
        vm.pet_render.generated_species = Species::Fuzz;
        vm.pet_render.stage = Stage::S3;
        vm.pet_art = rendered.lines;
        vm.pet_spans = rendered.spans;
        vm
    }

    fn project(
        vm: &WatchViewModel,
        width_points: f32,
        height_points: f32,
        wall_time: time::OffsetDateTime,
    ) -> CompanionSceneSnapshot {
        CompanionSceneSnapshot::project_with_input(
            vm,
            CompanionSceneProjectionInput::round(
                CompanionProjectionClock::new(wall_time, 0),
                CompanionLogicalLayout::round(width_points, height_points),
                COLUMNS,
                ROWS,
                glorp::round::scene::current_round_motion_clearance(ROWS),
            ),
        )
        .expect("project full retained composition")
    }

    fn intersects(left: [f32; 4], right: [f32; 4]) -> bool {
        left[0] < right[2] && left[2] > right[0] && left[1] < right[3] && left[3] > right[1]
    }

    let vm = full_cast_fixture();
    for &(width_points, height_points) in SURFACES {
        let first = project(
            &vm,
            width_points,
            height_points,
            datetime!(2026-07-15 12:00:00 UTC),
        );
        let repeated = project(
            &vm,
            width_points,
            height_points,
            datetime!(2026-07-15 12:00:00 UTC),
        );
        let next_phase = project(
            &vm,
            width_points,
            height_points,
            datetime!(2026-07-15 12:00:04 UTC),
        );
        let label = format!("{width_points}x{height_points}");

        assert_eq!(
            serde_json::to_vec(&first).unwrap(),
            serde_json::to_vec(&repeated).unwrap(),
            "{label} repeated projection"
        );
        assert!(
            first
                .content
                .prop_animation_states
                .iter()
                .zip(&next_phase.content.prop_animation_states)
                .any(|(left, right)| left.sprite_phase != right.sprite_phase),
            "{label} fixture must cross a semantic sprite phase"
        );

        let cell = first.topology.glyph_grid.cell_extent_points;
        let aperture_radius = f64::from(width_points.min(height_points)) / 2.0;
        let outer_inset = 3.0_f64.max(aperture_radius * 0.012);
        let xp_width = (aperture_radius * 0.050).clamp(6.0, 16.0);
        let daily_width = (aperture_radius * 0.040).clamp(5.0, 13.0);
        let pace_width = (aperture_radius * 0.034).clamp(4.0, 11.0);
        let lane_gap = (aperture_radius * 0.010).clamp(1.5, 4.0);
        let xp_radius = aperture_radius - outer_inset - xp_width / 2.0;
        let daily_radius = xp_radius - xp_width / 2.0 - lane_gap - daily_width / 2.0;
        let pace_radius = daily_radius - daily_width / 2.0 - lane_gap - pace_width / 2.0;
        let inner_radius = pace_radius - pace_width / 2.0;
        let safe_radii = [
            inner_radius as f32 / cell[0] - 0.5,
            inner_radius as f32 / cell[1] - 0.5,
        ];
        let center = [f32::from(COLUMNS) / 2.0, f32::from(ROWS) / 2.0];
        let hud = [
            (f32::from(COLUMNS) * 0.21).floor(),
            (f32::from(ROWS) * 0.58).floor(),
            (f32::from(COLUMNS) * 0.79).ceil(),
            (f32::from(ROWS) * 0.90).ceil(),
        ];
        let bottom = [
            0.0,
            f32::from(
                ROWS - glorp::round::scene::current_round_motion_clearance(ROWS)
                    .bottom_reserved_rows,
            ),
            f32::from(COLUMNS),
            f32::from(ROWS),
        ];
        let visible_bounds = first
            .frame
            .prop_instances
            .iter()
            .filter(|frame| frame.visible)
            .map(|frame| {
                assert!(
                    frame
                        .origin_points
                        .into_iter()
                        .chain(frame.footprint_points)
                        .all(f32::is_finite),
                    "{label} slot {} has non-finite footprint",
                    frame.slot
                );
                assert!(frame.footprint_points.into_iter().all(|value| value > 0.0));
                let min = [
                    frame.origin_points[0] / cell[0],
                    frame.origin_points[1] / cell[1],
                ];
                let max = [
                    min[0] + frame.footprint_points[0] / cell[0],
                    min[1] + frame.footprint_points[1] / cell[1],
                ];
                let bounds = [min[0], min[1], max[0], max[1]];
                for col in [bounds[0] + 0.5, bounds[2] - 0.5] {
                    for row in [bounds[1] + 0.5, bounds[3] - 0.5] {
                        let dx = (col - center[0]) / safe_radii[0];
                        let dy = (row - center[1]) / safe_radii[1];
                        assert!(
                            dx * dx + dy * dy <= 1.0 + f32::EPSILON,
                            "{label} slot {} escaped the gauge-safe aperture",
                            frame.slot
                        );
                    }
                }
                assert!(
                    !intersects(bounds, hud),
                    "{label} slot {} overlaps HUD",
                    frame.slot
                );
                assert!(
                    !intersects(bounds, bottom),
                    "{label} slot {} overlaps bottom reserve",
                    frame.slot
                );
                bounds
            })
            .collect::<Vec<_>>();
        for (index, left) in visible_bounds.iter().copied().enumerate() {
            for right in visible_bounds[index + 1..].iter().copied() {
                assert!(
                    !intersects(left, right),
                    "{label} visible footprints overlap"
                );
            }
        }

        let placement = |snapshot: &CompanionSceneSnapshot| {
            snapshot
                .frame
                .prop_instances
                .iter()
                .map(|frame| {
                    (
                        frame.slot,
                        frame.visible,
                        frame.origin_points,
                        frame.footprint_points,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            placement(&first),
            placement(&next_phase),
            "{label} phase repacked props"
        );
        let hidden_accents = |snapshot: &CompanionSceneSnapshot| {
            snapshot
                .frame
                .prop_instances
                .iter()
                .filter(|frame| !frame.visible)
                .filter(|frame| {
                    let id = snapshot.topology.visible_props[usize::from(frame.slot)].catalog_id;
                    HABITAT_PROP_CATALOG
                        .iter()
                        .find(|spec| spec.id == id)
                        .is_some_and(|spec| spec.kind == HabitatPropKind::Accent)
                })
                .map(|frame| frame.slot)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            hidden_accents(&first),
            hidden_accents(&repeated),
            "{label} hidden accents"
        );
        assert_eq!(
            hidden_accents(&first),
            hidden_accents(&next_phase),
            "{label} phase hiding"
        );
    }
}
