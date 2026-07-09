use glorp::game::habitat::HabitatPropKind;
use glorp::presentation::smooth::{SmoothCompanionPrivacyClaims, SmoothLayerItem, SmoothLayerRole};
use glorp::round::scene::{build_round_scene_draw_list, CompanionMotion};
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
fn smooth_round_plan_flattens_to_classic_round_scene_for_fixed_fixture() {
    let vm = parity_fixture();
    let motion = CompanionMotion::default();

    let classic = build_round_scene_draw_list(&vm, NOW, GRID_COLS, GRID_ROWS, &motion);
    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, NOW, GRID_COLS, GRID_ROWS, &motion, 0,
    );

    assert_eq!(smooth.flatten_classic_cells(), classic.draw_list);
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
            SmoothLayerRole::ContactShadow,
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
fn smooth_round_plan_records_fractional_pet_anchors_without_breaking_flatten_parity() {
    let vm = parity_fixture();
    let motion = glorp::round::scene::companion_roam_motion();
    let now = datetime!(2026-07-08 18:00:00.500 UTC);

    let classic = build_round_scene_draw_list(&vm, now, GRID_COLS, GRID_ROWS, &motion);
    let smooth = glorp::round::smooth::build_round_smooth_scene_plan(
        &vm, now, GRID_COLS, GRID_ROWS, &motion, 250,
    );

    assert_eq!(smooth.flatten_classic_cells(), classic.draw_list);
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
fn smooth_round_plan_moves_pet_attached_layers_but_keeps_chest_bubble_snapped() {
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
    let contact_shadow = plan.layer_by_role(SmoothLayerRole::ContactShadow).unwrap();
    let performance_cue = plan.layer_by_role(SmoothLayerRole::PerformanceCue).unwrap();
    let chest_bubble = plan.layer_by_role(SmoothLayerRole::ChestBubble).unwrap();

    assert!(pet_body.transform.translation.x.abs() > f32::EPSILON);
    assert_eq!(
        contact_shadow.transform.translation.x,
        pet_body.transform.translation.x
    );
    assert_eq!(
        performance_cue.transform.translation.x,
        pet_body.transform.translation.x
    );
    assert_eq!(chest_bubble.transform.translation.x, 0.0);
    assert_eq!(chest_bubble.transform.translation.y, 0.0);
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
