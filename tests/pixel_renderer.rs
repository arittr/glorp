use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    render_pixel_frame, PixelPetInput, PixelRendererState, PixelRendererTick, PixelViewport,
};
use glorp::tui::view_model::WatchViewModel;
use time::macros::datetime;

fn frame_for(vm: &WatchViewModel, ms: i64) -> glorp::presentation::pixel::PixelFrame {
    let base = datetime!(2026-07-08 12:00 UTC);
    let now = base + time::Duration::milliseconds(ms);
    let input = PixelPetInput::from_watch_view_model(vm, now);
    let mut state = PixelRendererState::new(&input, base);
    render_pixel_frame(PixelRendererTick {
        input: &input,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    })
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
        let input = PixelPetInput::from_watch_view_model(&vm, now);
        let frame_a = render_pixel_frame(PixelRendererTick {
            input: &input,
            viewport: PixelViewport::companion_default(),
            now,
            state: &mut state_a,
        });
        let frame_b = render_pixel_frame(PixelRendererTick {
            input: &input,
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
