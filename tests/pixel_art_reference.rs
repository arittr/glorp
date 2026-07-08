use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    PixelArtReferenceProvider, PixelArtRole, PixelCanonicalAnimationInputs, PixelPetArtReference,
    PixelPetInput,
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

fn orthogonal_neighbor_count(reference: &PixelPetArtReference, x: u8, y: u8) -> usize {
    let footprint = reference
        .occupied_cells
        .iter()
        .map(|cell| (i16::from(cell.x), i16::from(cell.y)))
        .collect::<std::collections::BTreeSet<_>>();
    let x = i16::from(x);
    let y = i16::from(y);
    [(-1, 0), (1, 0), (0, -1), (0, 1)]
        .into_iter()
        .filter(|(dx, dy)| footprint.contains(&(x + dx, y + dy)))
        .count()
}

fn assert_region_matches_role_cells(
    reference: &PixelPetArtReference,
    region_id: &str,
    expected_role: &str,
    cells: &[glorp::presentation::pixel::PixelArtCell],
) {
    let region = reference
        .protected_region(region_id)
        .unwrap_or_else(|| panic!("missing protected region: {region_id}"));
    assert_eq!(region.role, expected_role);
    assert!(
        !cells.is_empty(),
        "{region_id} must cover at least one promoted cell"
    );
    assert_eq!(region.cell_count, cells.len(), "{region_id} cell count");
    for cell in cells {
        assert!(
            cell.x >= region.bounds.min_x
                && cell.x <= region.bounds.max_x
                && cell.y >= region.bounds.min_y
                && cell.y <= region.bounds.max_y,
            "{region_id} bounds must cover promoted cell {cell:?}"
        );
    }
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
fn canonical_art_request_preserves_explicit_animation_inputs() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut vm = WatchViewModel::fixture();
    vm.day_context.asleep = true;
    let explicit = PixelCanonicalAnimationInputs {
        tick: 17,
        hold_eyes_closed: false,
        blink_suppression_ticks: 3,
    };

    let (_input, request) =
        PixelPetInput::from_watch_view_model_with_canonical_art_request(&vm, now, explicit);
    let mut provider = PixelArtReferenceProvider::default();
    let reference = provider.reference_for(&request);

    assert_eq!(request.animation_frame.tick, explicit.tick);
    assert_eq!(
        request.animation_frame.hold_eyes_closed,
        explicit.hold_eyes_closed
    );
    assert_eq!(
        request.animation_frame.blink_suppression_ticks,
        explicit.blink_suppression_ticks
    );
    assert_eq!(reference.pose.tick, explicit.tick);
    assert_eq!(reference.pose.hold_eyes_closed, explicit.hold_eyes_closed);
    assert_eq!(
        reference.pose.blink_suppression_ticks,
        explicit.blink_suppression_ticks
    );
    assert!(
        !reference
            .reference_checksum
            .eq(&reference_for(&vm, 0).reference_checksum),
        "explicit canonical animation inputs should affect the reference checksum"
    );
}

#[test]
fn blink_suppression_ticks_get_distinct_cached_references() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let vm = WatchViewModel::fixture();
    let mut provider = PixelArtReferenceProvider::default();
    let base = PixelCanonicalAnimationInputs {
        tick: 1,
        hold_eyes_closed: false,
        blink_suppression_ticks: 0,
    };
    let suppressed = PixelCanonicalAnimationInputs { blink_suppression_ticks: 1, ..base };

    let (_input, base_request) =
        PixelPetInput::from_watch_view_model_with_canonical_art_request(&vm, now, base);
    let (_input, suppressed_request) =
        PixelPetInput::from_watch_view_model_with_canonical_art_request(&vm, now, suppressed);

    let first = provider.reference_for(&base_request);
    let second = provider.reference_for(&suppressed_request);

    assert_ne!(
        first.reference_checksum, second.reference_checksum,
        "blink suppression changes render-affecting animation state and must not alias"
    );
    assert_ne!(first.pose, second.pose);
    assert_eq!(provider.render_count_for_test(), 2);
}

#[test]
fn glitch_day_seed_gets_distinct_cached_references() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut first_vm = WatchViewModel::fixture();
    first_vm.pet_render.generated_species = Species::Glitch;
    first_vm.pet_render.stage = Stage::S4;
    first_vm.pet_render.mood = Mood::Content;
    first_vm.life_profile.burst_level = 0.9;
    first_vm.last_feed_pulse_at = Some(now - time::Duration::milliseconds(300));
    first_vm.day_context.date_seed = 42;

    let mut second_vm = first_vm.clone();
    second_vm.day_context.date_seed = 777;

    let inputs = PixelCanonicalAnimationInputs {
        tick: 9,
        hold_eyes_closed: false,
        blink_suppression_ticks: 0,
    };
    let (_input, first_request) =
        PixelPetInput::from_watch_view_model_with_canonical_art_request(&first_vm, now, inputs);
    let (_input, second_request) =
        PixelPetInput::from_watch_view_model_with_canonical_art_request(&second_vm, now, inputs);
    let first_uncached = PixelArtReferenceProvider::default().reference_for(&first_request);
    let second_uncached = PixelArtReferenceProvider::default().reference_for(&second_request);
    let serialized = serde_json::to_string(&first_uncached).unwrap();
    assert_ne!(
        first_uncached.reference_checksum, second_uncached.reference_checksum,
        "glitch day seed changes canonical repair marks and must affect the reference"
    );
    assert!(
        !serialized.contains("day_seed"),
        "serialized art references must not expose raw date seeds: {serialized}"
    );
    assert!(
        !serialized.contains("glitch_day_key"),
        "serialized art references must not expose cache-only date keys: {serialized}"
    );

    let mut provider = PixelArtReferenceProvider::default();
    let first = provider.reference_for(&first_request);
    let second = provider.reference_for(&second_request);

    assert_eq!(first.reference_checksum, first_uncached.reference_checksum);
    assert_eq!(
        second.reference_checksum,
        second_uncached.reference_checksum
    );
    assert_ne!(first.pose, second.pose);
    assert_eq!(provider.render_count_for_test(), 2);
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
    assert_region_matches_role_cells(&reference, "signature-locket", "signature", &locket_cells);
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
    assert_region_matches_role_cells(&reference, "signature-facet", "signature", &facet_cells);
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
    let coverage = reference
        .cue_coverage("repair_mark")
        .expect("repair coverage");

    assert!(!repair_cells.is_empty(), "repair cells must be promoted");
    assert_eq!(coverage.expected, coverage.present);
    assert!(face_cells.iter().all(|cell| !repair_cells.contains(cell)));
    assert_region_matches_role_cells(&reference, "face", "face", &face_cells);
    assert_region_matches_role_cells(
        &reference,
        "signature-repair-mark",
        "signature",
        &repair_cells,
    );
}

#[test]
fn mech_s3_promotes_outline_appendage_and_foot_contact_as_real_cells() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Mech;
    vm.pet_render.stage = Stage::S3;
    vm.pet_render.mood = Mood::Content;

    let reference = reference_for(&vm, 0);

    assert!(!reference
        .cells_for_roles([PixelArtRole::Outline])
        .is_empty());
    assert!(!reference
        .cells_for_roles([PixelArtRole::Appendage])
        .is_empty());
    let visible_foot_contact = reference.cells_for_roles([PixelArtRole::FootContact]);
    assert!(!visible_foot_contact.is_empty());
    assert!(
        visible_foot_contact
            .iter()
            .all(|cell| reference.foot_contact.cells.contains(&(cell.x, cell.y))),
        "visible foot-contact cells must come from foot-contact evidence"
    );
    assert!(
        reference.foot_contact.cells.len() >= visible_foot_contact.len(),
        "foot-contact evidence may include cells whose visible role has higher priority"
    );
}

#[test]
fn mech_s5_does_not_classify_two_neighbor_chassis_cells_as_appendages() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Mech;
    vm.pet_render.stage = Stage::S5;
    vm.pet_render.mood = Mood::Content;

    let reference = reference_for(&vm, 0);
    let appendage_cells = reference.cells_for_roles([PixelArtRole::Appendage]);

    assert!(
        appendage_cells
            .iter()
            .all(|cell| orthogonal_neighbor_count(&reference, cell.x, cell.y) <= 1),
        "appendage cells must use the narrow <=1 orthogonal-neighbor heuristic: {appendage_cells:?}"
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
