use glorp::game::habitat::HabitatPropKind;
use glorp::presentation::smooth::{
    SmoothCompanionPrivacyClaims, SmoothDepthPlane, SmoothLayerItem, SmoothLayerMotionBinding,
    SmoothLayerRole,
};
use glorp::round::scene::CompanionMotion;
use glorp::storage::state::{HabitatPropId, HabitatPropSource};
use glorp::tui::view_model::{EarnedHabitatPropView, SourceStatus, WatchViewModel};
use time::macros::datetime;

const GRID_COLS: u16 = 44;
const GRID_ROWS: u16 = 18;
const NOW: time::OffsetDateTime = datetime!(2026-06-13 18:00 UTC);

fn parity_fixture() -> WatchViewModel {
    let mut vm = WatchViewModel::fixture_with_habitat_props();
    vm.source_health[0].status = SourceStatus::Diagnostic;
    vm.habitat.earned_props.push(EarnedHabitatPropView {
        id: HabitatPropId::new(glorp::game::habitat::TOKEN_TREASURE_CHEST_2M),
        earned_at: time::OffsetDateTime::UNIX_EPOCH,
        kind: HabitatPropKind::Trophy,
        display_priority: 100,
        source: HabitatPropSource::LifetimeTokens { threshold: 2_000_000.0 },
    });
    vm
}

#[test]
fn smooth_plan_focus_is_continuous_wander_minus_neutral_origin() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);
    let placement =
        glorp::round::scene::companion_pet_placement(&vm, now, GRID_COLS, GRID_ROWS, &motion);
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 500,
    );

    assert_eq!(
        plan.pet.parallax_focus_offset,
        glorp::presentation::smooth::SmoothPoint {
            x: placement.fractional_motion_top_left.x
                - placement.fractional_motion_origin_top_left.x,
            y: placement.fractional_motion_top_left.y
                - placement.fractional_motion_origin_top_left.y,
        }
    );
}

#[test]
fn smooth_plan_assigns_every_current_role_its_approved_binding() {
    use SmoothDepthPlane::{Behind, Far, Foreground, Mid};
    use SmoothLayerMotionBinding::{Fixed, FloorProjected, Parallax, PetAttached};
    use SmoothLayerRole::*;

    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &glorp::round::scene::companion_roam_motion(),
        0,
    );
    let expected = [
        (DepthRings, Fixed),
        (BiomeWash, Parallax(Far)),
        (RoomGlyphs, Parallax(Far)),
        (Ambient, Parallax(Mid)),
        (Motes, Parallax(Mid)),
        (ActivityGlyphs, Parallax(Mid)),
        (PropsBehind, Parallax(Behind)),
        (TankLifeBehind, Parallax(Behind)),
        (ChestBubble, Parallax(Behind)),
        (WallShadow, PetAttached),
        (FloorProjection, FloorProjected),
        (PetBody, PetAttached),
        (PerformanceCue, PetAttached),
        (PropsForeground, Parallax(Foreground)),
        (TankLifeForeground, Parallax(Foreground)),
        (StatusHalo, Fixed),
        (TroubleIndicator, Fixed),
        (MoodAura, PetAttached),
        (DimOverlay, Fixed),
    ];

    for (role, binding) in expected {
        let layer = plan.layer_by_role(role).expect("current role should exist");
        assert_eq!(
            layer.motion_binding, binding,
            "unexpected binding for {role:?}"
        );
    }
}

#[test]
fn smooth_plan_lifecycle_scale_uses_asleep_precedence() {
    let motion = glorp::round::scene::companion_roam_motion();
    let mut normal = parity_fixture();
    normal.day_context.asleep = false;
    normal.life_profile.calm_mode = false;
    let mut calm = normal.clone();
    calm.life_profile.calm_mode = true;
    let mut asleep_and_calm = calm.clone();
    asleep_and_calm.day_context.asleep = true;

    let normal_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &normal, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let calm_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &calm, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let asleep_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &asleep_and_calm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &motion,
        0,
    );

    assert_eq!(normal_plan.parallax_lifecycle_scale, 1.0);
    assert_eq!(calm_plan.parallax_lifecycle_scale, 0.5);
    assert_eq!(asleep_plan.parallax_lifecycle_scale, 0.25);
}

#[test]
fn fallible_smooth_round_plan_matches_existing_infallible_plan() {
    let vm = parity_fixture();
    let motion = CompanionMotion::default();

    let infallible = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 250,
    );
    let fallible = glorp::round::smooth::try_build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 250,
    )
    .expect("fixture should include pet body layer");

    assert_eq!(fallible, infallible);
}

fn anchored_bounds(
    anchor: glorp::presentation::smooth::SmoothPoint,
    local_bounds: glorp::presentation::smooth::SmoothBounds,
) -> glorp::presentation::smooth::SmoothBounds {
    glorp::presentation::smooth::SmoothBounds {
        min: glorp::presentation::smooth::SmoothPoint {
            x: anchor.x + local_bounds.min.x,
            y: anchor.y + local_bounds.min.y,
        },
        max: glorp::presentation::smooth::SmoothPoint {
            x: anchor.x + local_bounds.max.x,
            y: anchor.y + local_bounds.max.y,
        },
    }
}

#[test]
fn smooth_round_plan_includes_classic_and_round_only_roles() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        0,
    );
    let roles: Vec<_> = plan.layers.iter().map(|layer| layer.role).collect();

    assert_eq!(
        roles,
        vec![
            SmoothLayerRole::DepthRings,
            SmoothLayerRole::BiomeWash,
            SmoothLayerRole::RoomGlyphs,
            SmoothLayerRole::Ambient,
            SmoothLayerRole::Motes,
            SmoothLayerRole::ActivityGlyphs,
            SmoothLayerRole::PropsBehind,
            SmoothLayerRole::TankLifeBehind,
            SmoothLayerRole::ChestBubble,
            SmoothLayerRole::WallShadow,
            SmoothLayerRole::FloorProjection,
            SmoothLayerRole::PetBody,
            SmoothLayerRole::PerformanceCue,
            SmoothLayerRole::PropsForeground,
            SmoothLayerRole::TankLifeForeground,
            SmoothLayerRole::StatusHalo,
            SmoothLayerRole::TroubleIndicator,
            SmoothLayerRole::MoodAura,
            SmoothLayerRole::DimOverlay,
        ]
    );
}

#[test]
fn smooth_round_plan_gives_pet_body_a_fractional_bob_transform() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        250,
    );
    let pet_body = plan
        .layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .expect("pet body layer should exist");

    assert!(pet_body.transform.translation.y != 0.0);
    assert!(pet_body.transform.translation.y.fract() != 0.0);
}

#[test]
fn smooth_round_plan_keeps_classic_cell_art_in_pet_body() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        250,
    );
    let pet_body = plan
        .layers
        .iter()
        .find(|layer| layer.role == SmoothLayerRole::PetBody)
        .expect("pet body layer should exist");

    assert!(!pet_body.items.is_empty());
    assert!(pet_body
        .items
        .iter()
        .all(|item| matches!(item, SmoothLayerItem::LocalCell(_))));
}

#[test]
fn smooth_round_plan_records_fractional_pet_anchors() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);

    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 250,
    );

    assert_eq!(smooth.pet.bounds.min.x, smooth.pet.classic_snap_anchor.x);
    assert_eq!(smooth.pet.bounds.min.y, smooth.pet.classic_snap_anchor.y);
    assert!(
        (smooth.pet.base_anchor.x - smooth.pet.classic_snap_anchor.x).abs() > f32::EPSILON
            || (smooth.pet.base_anchor.y - smooth.pet.classic_snap_anchor.y).abs() > f32::EPSILON,
        "smooth plan should preserve fractional residual separate from Classic snap"
    );
    assert_ne!(smooth.pet.final_anchor, smooth.pet.base_anchor);
}

#[test]
fn smooth_round_plan_floor_projection_stays_below_props_and_moves_pet_attached_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        250,
    );
    let pet_body = plan.layer_by_role(SmoothLayerRole::PetBody).unwrap();
    let wall_shadow = plan.layer_by_role(SmoothLayerRole::WallShadow).unwrap();
    let floor_projection = plan
        .layer_by_role(SmoothLayerRole::FloorProjection)
        .unwrap();
    let performance_cue = plan.layer_by_role(SmoothLayerRole::PerformanceCue).unwrap();
    let chest_bubble = plan.layer_by_role(SmoothLayerRole::ChestBubble).unwrap();
    let props_behind = plan.layer_by_role(SmoothLayerRole::PropsBehind).unwrap();
    let first_prop_z = plan
        .layers
        .iter()
        .filter(|layer| {
            matches!(
                layer.role,
                SmoothLayerRole::PropsBehind
                    | SmoothLayerRole::PropsForeground
                    | SmoothLayerRole::TankLifeBehind
                    | SmoothLayerRole::TankLifeForeground
            )
        })
        .map(|layer| layer.z)
        .min()
        .unwrap();

    assert!(pet_body.transform.translation.x.abs() > f32::EPSILON);
    assert_eq!(
        wall_shadow.transform.translation.x,
        pet_body.transform.translation.x
    );
    assert_eq!(
        floor_projection.transform.translation.x,
        pet_body.transform.translation.x
    );
    assert_eq!(
        floor_projection.transform.translation.y, -1.0,
        "the floor projection is lifted one substrate row and must not inherit pet bob"
    );
    assert!(floor_projection.z < first_prop_z);
    assert!(!plan
        .layers
        .iter()
        .any(|layer| layer.role.as_str() == "floor-texture"));
    for prop_role in [
        SmoothLayerRole::PropsBehind,
        SmoothLayerRole::TankLifeBehind,
        SmoothLayerRole::ChestBubble,
        SmoothLayerRole::PropsForeground,
        SmoothLayerRole::TankLifeForeground,
    ] {
        let prop_layer = plan.layer_by_role(prop_role).unwrap();
        assert!(
            floor_projection.z < prop_layer.z,
            "floor projection must stay below {prop_role:?}"
        );
    }
    assert_eq!(
        performance_cue.transform.translation.x,
        pet_body.transform.translation.x
    );
    assert_eq!(
        chest_bubble.motion_binding,
        SmoothLayerMotionBinding::Parallax(SmoothDepthPlane::Behind)
    );
    assert_eq!(chest_bubble.motion_binding, props_behind.motion_binding);
    assert_eq!(
        chest_bubble.transform.translation,
        chest_bubble.parallax_translation
    );
}

#[test]
fn smooth_plan_composes_nonzero_parallax_without_moving_fixed_or_pet_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        500,
    );

    assert_ne!(
        plan.pet.parallax_focus_offset,
        glorp::presentation::smooth::SmoothPoint::default()
    );
    assert!(plan.layers.iter().any(|layer| {
        matches!(layer.motion_binding, SmoothLayerMotionBinding::Parallax(_))
            && layer.parallax_translation != glorp::presentation::smooth::SmoothPoint::default()
    }));
    for layer in &plan.layers {
        if matches!(
            layer.motion_binding,
            SmoothLayerMotionBinding::Fixed
                | SmoothLayerMotionBinding::PetAttached
                | SmoothLayerMotionBinding::FloorProjected
        ) {
            assert_eq!(
                layer.parallax_translation,
                glorp::presentation::smooth::SmoothPoint::default(),
                "fixed and pet-attached layers must not receive parallax: {:?}",
                layer.role
            );
        }
    }
}

#[test]
fn nonzero_parallax_is_preserved_for_smooth_layers() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);
    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 500,
    );

    assert!(smooth.layers.iter().any(|layer| {
        layer.parallax_translation != glorp::presentation::smooth::SmoothPoint::default()
    }));
}

#[test]
fn classic_breath_does_not_change_parallax_focus() {
    let motion = glorp::round::scene::companion_roam_motion();
    let mut still = parity_fixture();
    let mut breathed = still.clone();
    still.breath_offset_y = 0;
    breathed.breath_offset_y = 1;

    let still_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &still, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );
    let breathed_plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &breathed, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );

    assert_eq!(
        still_plan.pet.parallax_focus_offset,
        breathed_plan.pet.parallax_focus_offset
    );
}

#[test]
fn smooth_round_plan_uses_posture_shifted_pet_body_for_metadata_and_aura() {
    let mut vm = parity_fixture();
    vm.day_context.asleep = true;
    let motion = glorp::round::scene::companion_roam_motion();
    let elapsed_ms = 250;
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        datetime!(2026-07-08 18:00:00.500 UTC),
        GRID_COLS,
        GRID_ROWS,
        &motion,
        elapsed_ms,
    );
    let pet_body = plan.layer_by_role(SmoothLayerRole::PetBody).unwrap();
    let mood_aura = plan.layer_by_role(SmoothLayerRole::MoodAura).unwrap();
    let snapped_bounds = anchored_bounds(pet_body.anchor, pet_body.local_bounds);
    let fractional_anchor = glorp::presentation::smooth::SmoothPoint {
        x: pet_body.anchor.x + pet_body.transform.translation.x,
        y: pet_body.anchor.y + pet_body.transform.translation.y,
    };
    let fractional_bounds = anchored_bounds(fractional_anchor, pet_body.local_bounds);
    let fractional_center = glorp::presentation::smooth::SmoothPoint {
        x: (fractional_bounds.min.x + fractional_bounds.max.x) / 2.0,
        y: (fractional_bounds.min.y + fractional_bounds.max.y) / 2.0,
    };

    assert_eq!(plan.pet.classic_snap_anchor, pet_body.anchor);
    assert_eq!(plan.pet.bounds, snapped_bounds);
    assert_eq!(
        plan.pet.base_anchor.x,
        pet_body.anchor.x + pet_body.transform.translation.x
    );
    assert_eq!(
        plan.pet.base_anchor.y,
        pet_body.anchor.y + pet_body.transform.translation.y - plan.pet.bob_offset.y
    );
    assert_eq!(plan.pet.final_anchor, fractional_anchor);
    assert_eq!(plan.pet.fractional_bounds, fractional_bounds);
    assert_eq!(mood_aura.transform_origin, fractional_center);
}

#[test]
fn smooth_round_plan_limits_adjacent_paint_frame_anchor_delta() {
    let start = datetime!(2026-07-08 12:00 UTC);
    const COMPANION_GRID_COLS: u16 = 36;
    const COMPANION_GRID_ROWS: u16 = 18;
    let motion = glorp::round::scene::companion_roam_motion();
    let mut vm = parity_fixture();
    vm.pet_render.generated_species = glorp::pet::generation::Species::Glitch;
    vm.pet_render.stage = glorp::game::evolution::Stage::S4;
    vm.life_profile.burst_level = 1.0;
    vm.progress.rate_per_hour = 75_000_000.0;
    glorp::commands::watch::rerender_pet_for_view_model(&mut vm, 0, false, start).unwrap();

    let mut previous: Option<glorp::presentation::smooth::SmoothPoint> = None;
    let mut max_delta = 0.0f32;
    for frame in 0..(22 * 30) {
        let elapsed_ms = frame * 33;
        let now = start + time::Duration::milliseconds(elapsed_ms);
        let plan = glorp::round::smooth::build_round_smooth_scene_plan(
            &vm,
            now,
            COMPANION_GRID_COLS,
            COMPANION_GRID_ROWS,
            &motion,
            elapsed_ms as u64,
        );
        if let Some(last) = previous {
            let dx = (plan.pet.base_anchor.x - last.x).abs();
            let dy = (plan.pet.base_anchor.y - last.y).abs();
            max_delta = max_delta.max(dx).max(dy);
        }
        previous = Some(plan.pet.base_anchor);
    }

    assert!(
        max_delta <= 0.25,
        "adjacent paint frame anchors should stay smooth; max delta was {max_delta:.3}"
    );
}

#[test]
fn smooth_round_plan_does_not_turn_classic_breath_step_into_world_motion() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let motion = glorp::round::scene::companion_roam_motion();
    let mut still = parity_fixture();
    let mut breathed = still.clone();
    still.breath_offset_y = 0;
    breathed.breath_offset_y = 1;

    let still_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&still, now, 36, 18, &motion, 0);
    let breathed_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&breathed, now, 36, 18, &motion, 0);

    assert_ne!(
        still_plan.pet.classic_snap_anchor.y, breathed_plan.pet.classic_snap_anchor.y,
        "Classic snap should still preserve the discrete breath row"
    );
    assert_eq!(still_plan.pet.base_anchor, breathed_plan.pet.base_anchor);
    assert_eq!(still_plan.pet.final_anchor, breathed_plan.pet.final_anchor);
}

#[test]
fn smooth_round_plan_does_not_turn_performance_posture_step_into_world_motion() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let motion = glorp::round::scene::companion_roam_motion();
    let mut perked = parity_fixture();
    perked.life_profile.burst_level = 1.0;
    perked.last_feed_pulse_at = Some(now);
    let mut settled = perked.clone();
    settled.last_feed_pulse_at = None;
    settled.day_context.tiredness = 0.9;

    let perked_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&perked, now, 36, 18, &motion, 0);
    let settled_plan =
        glorp::round::smooth::build_round_smooth_scene_plan(&settled, now, 36, 18, &motion, 0);

    assert_ne!(
        perked_plan.pet.classic_snap_anchor.y, settled_plan.pet.classic_snap_anchor.y,
        "Classic snap should still preserve the settled posture row"
    );
    assert_eq!(perked_plan.pet.base_anchor, settled_plan.pet.base_anchor);
    assert_eq!(perked_plan.pet.final_anchor, settled_plan.pet.final_anchor);
}

#[test]
fn smooth_round_plan_keeps_privacy_claims_external_safe() {
    let vm = parity_fixture();
    let plan = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm,
        NOW,
        GRID_COLS,
        GRID_ROWS,
        &CompanionMotion::default(),
        250,
    );

    assert_eq!(
        plan.privacy,
        SmoothCompanionPrivacyClaims::external_companion()
    );
    assert!(plan
        .layers
        .iter()
        .all(|layer| layer.privacy == SmoothCompanionPrivacyClaims::external_companion()));
}
