use glorp::game::evolution::Stage;
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
