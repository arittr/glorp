use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    PixelArtReferenceProvider, PixelArtRole, PixelPetArtReference, PixelPetInput,
};
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

fn reference_for(vm: &WatchViewModel, ms: i64) -> PixelPetArtReference {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let (_input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
    let mut provider = PixelArtReferenceProvider::default();
    provider.reference_for(&request)
}

#[test]
fn fuzz_s3_reference_preserves_real_cast_cues() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;
    vm.pet_render.mood = Mood::Content;

    let reference = reference_for(&vm, 0);

    assert_eq!(reference.species, Species::Fuzz);
    assert_eq!(reference.stage, Stage::S3);
    assert!(reference.role_count(PixelArtRole::Eye) >= 2);
    assert!(reference.role_count(PixelArtRole::Locket) >= 1);
    assert!(reference.foot_contact.cells.len() >= 2);
    assert!(reference.body_bounds.width() >= 6);
    assert!(reference.body_bounds.height() >= 5);
    assert!(reference.occupied_cells.len() >= 30);
}

#[test]
fn glitch_s4_reference_preserves_repair_and_protected_face_roles() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Glitch;
    vm.pet_render.stage = Stage::S4;
    vm.pet_render.mood = Mood::Content;
    vm.life_profile.burst_level = 0.9;
    vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(300));

    let reference = reference_for(&vm, 300);

    assert_eq!(reference.species, Species::Glitch);
    assert_eq!(reference.stage, Stage::S4);
    assert!(reference.role_count(PixelArtRole::Eye) > 0);
    assert!(reference.role_count(PixelArtRole::Mouth) > 0);
    assert!(reference.role_count(PixelArtRole::RepairMark) > 0);
    assert!(reference.role_count(PixelArtRole::Corruption) > 0);
    for cell in reference.cells_for_roles([PixelArtRole::Eye, PixelArtRole::Mouth]) {
        assert!(
            !reference
                .cells_for_roles([PixelArtRole::Corruption])
                .contains(&cell),
            "face cell must not be transient corruption: {cell:?}"
        );
    }
}

#[test]
fn reference_pose_is_stable_across_continuous_pixel_elapsed_time() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;

    let first = reference_for(&vm, 0);
    let later_same_pose = reference_for(&vm, 33);

    assert_eq!(first.pose, later_same_pose.pose);
    assert_eq!(first.reference_checksum, later_same_pose.reference_checksum);
}

#[test]
fn reference_provider_caches_same_pose_request() {
    let base = datetime!(2026-07-08 12:00 UTC);
    let vm = WatchViewModel::fixture();
    let (_input, request) = PixelPetInput::from_watch_view_model_with_art_request(&vm, base);
    let mut provider = PixelArtReferenceProvider::default();

    let first = provider.reference_for(&request);
    let second = provider.reference_for(&request);

    assert_eq!(first, second);
    assert_eq!(provider.render_count_for_test(), 1);
}

#[test]
fn serialized_reference_does_not_leak_raw_seed_or_terminal_art() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.seed = "very-secret-seed".to_string();
    let reference = reference_for(&vm, 0);

    let json = serde_json::to_string(&reference).unwrap();

    assert!(!json.contains("very-secret-seed"));
    assert!(!json.contains("art_text"));
    assert!(!json.contains("/\\\\_/\\\\"));
    assert!(!json.contains("( o.o )"));
}
