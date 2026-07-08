use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    frame::{PixelFrame, Rgba8},
    raster::alpha_blend_pixel,
    render_pixel_frame, PixelArtPoseKey, PixelArtRole, PixelCellBounds, PixelFootContact,
    PixelPetArtReference, PixelPetInput, PixelReferenceChecksum, PixelRendererState,
    PixelRendererTick, PixelViewport,
};
use glorp::round::hud::companion_hud_text;
use glorp::round::pixel_fit::{pixel_companion_fit, PixelTargetGeometry};
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

fn frame_for_with_reference(
    vm: &WatchViewModel,
    ms: i64,
) -> (glorp::presentation::pixel::PixelFrame, PixelPetArtReference) {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
    let mut state = PixelRendererState::new(&input, base);
    let reference = state.art_reference_for(&request);
    let frame = render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    });
    (frame, reference)
}

fn frame_for(vm: &WatchViewModel, ms: i64) -> glorp::presentation::pixel::PixelFrame {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(vm, now);
    let mut state = PixelRendererState::new(&input, base);
    let reference = state.art_reference_for(&request);
    render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    })
}

fn frame_for_reference(vm: &WatchViewModel, reference: PixelPetArtReference) -> PixelFrame {
    let base = datetime!(2026-07-08 12:00 UTC);
    let input = PixelPetInput::from_watch_view_model(vm, base);
    let mut state = PixelRendererState::new(&input, base);
    render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &reference,
        viewport: PixelViewport::companion_default(),
        now: base,
        state: &mut state,
    })
}

fn reference_with_role_change(
    vm: &WatchViewModel,
    from: PixelArtRole,
    to: PixelArtRole,
) -> (PixelPetArtReference, PixelPetArtReference) {
    let (_frame, base_reference) = frame_for_with_reference(vm, 0);
    let mut changed = base_reference.clone();
    let cell_index = changed
        .occupied_cells
        .iter()
        .position(|cell| cell.role == from)
        .or_else(|| {
            (from == PixelArtRole::Body).then_some(())?;
            changed.occupied_cells.iter().position(|cell| {
                matches!(
                    cell.role,
                    PixelArtRole::BodyGlow
                        | PixelArtRole::Outline
                        | PixelArtRole::InteriorTexture
                        | PixelArtRole::Appendage
                        | PixelArtRole::FootContact
                ) && cell.role != to
            })
        })
        .expect("reference should contain source role");
    changed.occupied_cells[cell_index].role = to;
    (base_reference, changed)
}

fn frame_for_procedural_fallback(
    vm: &WatchViewModel,
    ms: i64,
) -> glorp::presentation::pixel::PixelFrame {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let input = PixelPetInput::from_watch_view_model(vm, now);
    let reference = PixelPetArtReference {
        species: input.identity.species,
        stage: input.identity.stage,
        mood: input.mood,
        pose: PixelArtPoseKey {
            tick: 0,
            hold_eyes_closed: false,
            blink_suppression_ticks: 0,
            blink_slowdown: 0,
            soft_eyes: false,
            work_accent: "none",
            feed_reaction: false,
            glitch_patch_tier: None,
            glitch_burst_level: None,
            glitch_day_key: None,
            glitch_calm_mode: false,
            glitch_feed_reaction: false,
        },
        width_cells: 0,
        height_cells: 0,
        occupied_cells: Vec::new(),
        body_bounds: PixelCellBounds { min_x: 0, min_y: 0, max_x: 0, max_y: 0 },
        foot_contact: PixelFootContact { cells: Vec::new() },
        protected_regions: Vec::new(),
        cue_coverage: std::collections::BTreeMap::new(),
        reference_checksum: PixelReferenceChecksum(0),
        role_counts: std::collections::BTreeMap::new(),
    };
    let mut state = PixelRendererState::new(&input, base);
    render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    })
}

#[test]
fn pixel_row_runs_coalesce_adjacent_equal_colors() {
    use glorp::presentation::pixel::{PixelFrame, PixelViewport, Rgba8};

    let mut frame = PixelFrame::transparent(PixelViewport { logical_width: 5, logical_height: 2 });
    let red = Rgba8::opaque(255, 0, 0);
    frame.set_pixel(1, 0, red);
    frame.set_pixel(2, 0, red);
    frame.set_pixel(4, 0, red);

    let runs = glorp::presentation::pixel::pixel_runs(&frame);

    assert_eq!(runs.len(), 2);
    assert_eq!(
        (runs[0].x, runs[0].y, runs[0].width, runs[0].color),
        (1, 0, 2, red)
    );
    assert_eq!(
        (runs[1].x, runs[1].y, runs[1].width, runs[1].color),
        (4, 0, 1, red)
    );
}

#[test]
fn pixel_row_runs_break_across_transparent_gaps() {
    use glorp::presentation::pixel::{PixelFrame, PixelViewport, Rgba8};

    let mut frame = PixelFrame::transparent(PixelViewport { logical_width: 5, logical_height: 1 });
    let red = Rgba8::opaque(255, 0, 0);
    frame.set_pixel(0, 0, red);
    frame.set_pixel(1, 0, red);
    frame.set_pixel(3, 0, red);
    frame.set_pixel(4, 0, red);

    let runs = glorp::presentation::pixel::pixel_runs(&frame);

    assert_eq!(runs.len(), 2);
    assert_eq!(
        (runs[0].x, runs[0].y, runs[0].width, runs[0].color),
        (0, 0, 2, red)
    );
    assert_eq!(
        (runs[1].x, runs[1].y, runs[1].width, runs[1].color),
        (3, 0, 2, red)
    );
}

#[test]
fn pixel_row_runs_reset_at_row_boundaries() {
    use glorp::presentation::pixel::{PixelFrame, PixelViewport, Rgba8};

    let mut frame = PixelFrame::transparent(PixelViewport { logical_width: 2, logical_height: 2 });
    let red = Rgba8::opaque(255, 0, 0);
    frame.set_pixel(0, 0, red);
    frame.set_pixel(1, 0, red);
    frame.set_pixel(0, 1, red);
    frame.set_pixel(1, 1, red);

    let runs = glorp::presentation::pixel::pixel_runs(&frame);

    assert_eq!(runs.len(), 2);
    assert_eq!(
        (runs[0].x, runs[0].y, runs[0].width, runs[0].color),
        (0, 0, 2, red)
    );
    assert_eq!(
        (runs[1].x, runs[1].y, runs[1].width, runs[1].color),
        (0, 1, 2, red)
    );
}

#[test]
fn pixel_row_runs_keep_adjacent_mixed_colors_separate() {
    use glorp::presentation::pixel::{PixelFrame, PixelViewport, Rgba8};

    let mut frame = PixelFrame::transparent(PixelViewport { logical_width: 4, logical_height: 1 });
    let red = Rgba8::opaque(255, 0, 0);
    let blue = Rgba8::opaque(0, 0, 255);
    frame.set_pixel(0, 0, red);
    frame.set_pixel(1, 0, red);
    frame.set_pixel(2, 0, blue);
    frame.set_pixel(3, 0, blue);

    let runs = glorp::presentation::pixel::pixel_runs(&frame);

    assert_eq!(runs.len(), 2);
    assert_eq!(
        (runs[0].x, runs[0].y, runs[0].width, runs[0].color),
        (0, 0, 2, red)
    );
    assert_eq!(
        (runs[1].x, runs[1].y, runs[1].width, runs[1].color),
        (2, 0, 2, blue)
    );
}

#[test]
fn pixel_renderer_is_deterministic_for_same_input_sequence() {
    let vm = WatchViewModel::fixture();
    let base = datetime!(2026-07-08 12:00 UTC);
    let mut state_a =
        PixelRendererState::new(&PixelPetInput::from_watch_view_model(&vm, base), base);
    let mut state_b =
        PixelRendererState::new(&PixelPetInput::from_watch_view_model(&vm, base), base);

    for ms in [0, 160, 320, 480, 640, 800, 960, 1_120] {
        let now = base + time::Duration::milliseconds(ms);
        let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(&vm, now);
        let reference_a = state_a.art_reference_for(&request);
        let reference_b = state_b.art_reference_for(&request);
        let frame_a = render_pixel_frame(PixelRendererTick {
            input: &input,
            art_reference: &reference_a,
            viewport: PixelViewport::companion_default(),
            now,
            state: &mut state_a,
        });
        let frame_b = render_pixel_frame(PixelRendererTick {
            input: &input,
            art_reference: &reference_b,
            viewport: PixelViewport::companion_default(),
            now,
            state: &mut state_b,
        });
        assert_eq!(frame_a, frame_b);
    }
}

#[test]
fn every_species_renders_non_empty_inside_the_frame() {
    for species in Species::all() {
        let mut vm = WatchViewModel::fixture();
        vm.pet_render.generated_species = species;
        vm.pet_render.stage = Stage::S3;
        vm.pet_render.mood = Mood::Content;
        let frame = frame_for(&vm, 0);

        assert!(
            frame.opaque_pixel_count() > 120,
            "{species} rendered too few pixels"
        );
        let bounds = frame
            .opaque_bounds()
            .expect("species should render visible pixels");
        assert!(bounds.max_x < frame.width);
        assert!(bounds.max_y < frame.height);
    }
}

#[test]
fn all_species_all_stages_render_reference_driven_frames() {
    const STAGES: [Stage; 7] = [
        Stage::S0,
        Stage::S1,
        Stage::S2,
        Stage::S3,
        Stage::S4,
        Stage::S5,
        Stage::S6,
    ];

    for species in Species::all() {
        for stage in STAGES {
            let mut vm = WatchViewModel::fixture();
            vm.pet_render.generated_species = species;
            vm.pet_render.stage = stage;
            vm.pet_render.mood = Mood::Content;

            let (frame, reference) = frame_for_with_reference(&vm, 500);

            assert!(
                !reference.occupied_cells.is_empty(),
                "{species:?} {stage:?} reference empty"
            );
            assert!(
                frame.opaque_pixel_count() > 40,
                "{species:?} {stage:?} frame empty"
            );
            let bounds = frame.opaque_bounds().expect("visible frame");
            assert!(bounds.max_x < frame.width);
            assert!(bounds.max_y < frame.height);
        }
    }
}

#[test]
fn hero_frame_uses_reference_roles_not_species_only_shape() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;
    vm.pet_render.mood = Mood::Content;

    let (frame, reference) = frame_for_with_reference(&vm, 480);

    assert!(reference.role_count(PixelArtRole::Locket) > 0);
    assert!(
        frame.changed_pixel_count(&frame_for_procedural_fallback(&vm, 480)) > 0,
        "reference-driven renderer should no longer match the old procedural-only helper"
    );
}

#[test]
fn signature_roles_change_visible_pixels() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Fuzz;
    vm.pet_render.stage = Stage::S3;

    let (base_reference, locket_reference) =
        reference_with_role_change(&vm, PixelArtRole::Body, PixelArtRole::Locket);
    let base_frame = frame_for_reference(&vm, base_reference);
    let locket_frame = frame_for_reference(&vm, locket_reference);

    assert!(
        base_frame.changed_pixel_count(&locket_frame) > 0,
        "locket role must change visible pixels"
    );
}

#[test]
fn structural_roles_change_visible_pixels() {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = Species::Mech;
    vm.pet_render.stage = Stage::S5;

    let (base_reference, outline_reference) =
        reference_with_role_change(&vm, PixelArtRole::Body, PixelArtRole::Outline);
    let base_frame = frame_for_reference(&vm, base_reference);
    let outline_frame = frame_for_reference(&vm, outline_reference);

    assert!(
        base_frame.changed_pixel_count(&outline_frame) > 0,
        "outline role must change visible pixels"
    );
}

#[test]
fn promoted_reference_roles_are_visible_in_hero_frames() {
    for (species, stage, required_role) in [
        (Species::Fuzz, Stage::S3, PixelArtRole::Locket),
        (Species::Glitch, Stage::S4, PixelArtRole::RepairMark),
        (Species::Crystal, Stage::S5, PixelArtRole::Facet),
        (Species::Mech, Stage::S5, PixelArtRole::Outline),
    ] {
        let mut vm = WatchViewModel::fixture();
        vm.pet_render.generated_species = species;
        vm.pet_render.stage = stage;
        let (frame, reference) = frame_for_with_reference(&vm, 480);

        assert!(
            reference.role_count(required_role) > 0,
            "{species:?} {stage:?} missing required promoted role {required_role:?}"
        );
        assert!(
            frame.opaque_pixel_count() > 120,
            "{species:?} {stage:?} rendered too few visible pixels"
        );
    }
}

#[test]
fn high_alpha_bounds_are_available_for_fit_checks() {
    let vm = WatchViewModel::fixture();
    let (frame, _reference) = frame_for_with_reference(&vm, 0);

    let bounds = frame.alpha_bounds(200).expect("high alpha body bounds");

    assert!(bounds.min_x <= bounds.max_x);
    assert!(bounds.min_y <= bounds.max_y);
}

#[test]
fn rendered_body_bounds_stay_above_the_hud_safe_zone() {
    let vm = WatchViewModel::fixture();
    let (frame, _reference) = frame_for_with_reference(&vm, 0);
    let body = frame.alpha_bounds(200).expect("high alpha body bounds");
    let hud = companion_hud_text(205_700_000.0, Some(9.99), 9_900_000.0);

    for size in [260_u16, 360, 480, 900] {
        let fit = pixel_companion_fit(
            PixelTargetGeometry { width: size, height: size },
            PixelViewport::companion_default(),
            &hud,
        );
        assert!(
            !fit.logical_bounds_overlap_hud(body),
            "rendered body overlapped HUD safe zone for {size}x{size}: {fit:?}"
        );
    }
}

#[test]
fn hero_fuzz_and_glitch_frames_are_visibly_different() {
    let mut fuzz = WatchViewModel::fixture();
    fuzz.pet_render.generated_species = Species::Fuzz;
    fuzz.pet_render.stage = Stage::S3;
    fuzz.pet_render.mood = Mood::Content;

    let mut glitch = fuzz.clone();
    glitch.pet_render.generated_species = Species::Glitch;
    glitch.pet_render.stage = Stage::S4;
    glitch.life_profile.burst_level = 0.8;
    glitch.last_feed_pulse_at = Some(datetime!(2026-07-08 11:59:59 UTC));

    let fuzz_frame = frame_for(&fuzz, 500);
    let glitch_frame = frame_for(&glitch, 500);

    assert!(fuzz_frame.changed_pixel_count(&glitch_frame) > 600);
}

#[test]
fn asleep_motion_amplitude_is_lower_than_idle() {
    let mut idle = WatchViewModel::fixture();
    idle.day_context.asleep = false;
    idle.life_profile.calm_mode = false;

    let mut asleep = idle.clone();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;

    let idle_a = frame_for(&idle, 0);
    let idle_b = frame_for(&idle, 800);
    let asleep_a = frame_for(&asleep, 0);
    let asleep_b = frame_for(&asleep, 800);

    assert!(idle_a.changed_pixel_count(&idle_b) > asleep_a.changed_pixel_count(&asleep_b));
}

#[test]
fn feed_pulse_changes_bounded_pixels() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let mut quiet = WatchViewModel::fixture();
    quiet.life_profile.burst_level = 0.0;
    quiet.last_feed_pulse_at = None;

    let mut pulsing = quiet.clone();
    pulsing.life_profile.burst_level = 0.9;
    pulsing.last_feed_pulse_at = Some(now - time::Duration::milliseconds(300));

    let quiet_frame = frame_for(&quiet, 300);
    let pulse_frame = frame_for(&pulsing, 300);
    let changed = quiet_frame.changed_pixel_count(&pulse_frame);

    assert!(changed > 80, "pulse should be visible");
    assert!(changed < 2_000, "pulse should stay bounded");
}

#[test]
fn alpha_blend_pixel_over_transparent_preserves_source_rgb() {
    let mut frame = PixelFrame::transparent(PixelViewport { logical_width: 1, logical_height: 1 });
    let source = Rgba8 { r: 180, g: 90, b: 30, a: 128 };

    alpha_blend_pixel(&mut frame, 0, 0, source);

    assert_eq!(frame.pixels[0], source);
}

#[test]
fn asleep_eye_pixels_stay_opaque_inside_the_body() {
    let base = datetime!(2026-07-08 12:00 UTC);
    let mut vm = WatchViewModel::fixture();
    vm.day_context.asleep = true;
    vm.life_profile.calm_mode = true;

    let input = PixelPetInput::from_watch_view_model(&vm, base);
    let wander_phase = f32::from(input.identity.variation_key.bucket(19)) * 0.17;
    let cx = 48_i16 + (wander_phase.sin() * 7.0 * 0.28).round() as i16;
    let cy = 48_i16;
    let frame = frame_for(&vm, 0);

    for x in [cx - 10, cx - 7, cx + 6, cx + 9] {
        let y = cy + 2;
        let idx = usize::from(y as u16) * usize::from(frame.width) + usize::from(x as u16);
        assert_eq!(
            frame.pixels[idx].a, 255,
            "asleep eye pixel at ({x}, {y}) should stay opaque inside the body"
        );
    }
}

#[test]
fn crystal_s0_asleep_eye_footprint_is_opaque_and_fully_drawn() {
    let base = datetime!(2026-07-08 12:00 UTC);
    let mut awake = WatchViewModel::fixture();
    awake.pet_render.generated_species = Species::Crystal;
    awake.pet_render.stage = Stage::S0;
    awake.day_context.asleep = false;
    awake.life_profile.calm_mode = false;

    let mut asleep = awake.clone();
    asleep.day_context.asleep = true;
    asleep.life_profile.calm_mode = true;

    let input = PixelPetInput::from_watch_view_model(&asleep, base);
    let wander_phase = f32::from(input.identity.variation_key.bucket(19)) * 0.17;
    let cx = 48_i16 + (wander_phase.sin() * 7.0 * 0.28).round() as i16;
    let cy = 48_i16;
    let awake_frame = frame_for(&awake, 0);
    let asleep_frame = frame_for(&asleep, 0);

    for x in [
        cx - 9,
        cx - 8,
        cx - 7,
        cx - 6,
        cx + 5,
        cx + 6,
        cx + 7,
        cx + 8,
    ] {
        let y = cy + 2;
        let idx = usize::from(y as u16) * usize::from(asleep_frame.width) + usize::from(x as u16);
        assert_eq!(
            asleep_frame.pixels[idx].a, 255,
            "crystal asleep eye pixel at ({x}, {y}) should stay opaque"
        );
        assert_ne!(
            asleep_frame.pixels[idx], awake_frame.pixels[idx],
            "crystal asleep eye pixel at ({x}, {y}) should be visibly drawn"
        );
    }
}
