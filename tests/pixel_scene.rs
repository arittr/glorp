use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    PixelFrame, PixelPetInput, PixelVariationKey, PixelViewport, Rgba8,
};
use glorp::tui::view_model::{SourceUsageView, WatchViewModel};
use time::macros::datetime;

#[test]
fn pixel_input_redacts_raw_seed_and_private_runtime_fields() {
    let mut vm = WatchViewModel::fixture_with_events();
    vm.pet_render.seed = "secret-seed-/Users/drew/project".to_string();
    vm.source_breakdown = vec![SourceUsageView {
        name: "client-source".into(),
        display_name: "private-client".into(),
        effective_tokens: 123_456.0,
    }];
    vm.helper_status = "helper failed at /Users/drew/private".into();
    vm.errors = vec!["prompt response diagnostic".into()];

    let input = PixelPetInput::from_watch_view_model(&vm, datetime!(2026-07-08 12:00 UTC));
    let debug = format!("{input:?}");

    assert!(!debug.contains("secret-seed"));
    assert!(!debug.contains("/Users/drew"));
    assert!(!debug.contains("client-source"));
    assert!(!debug.contains("private-client"));
    assert!(!debug.contains("123456"));
    assert!(!debug.contains("prompt"));
    assert!(!debug.contains("response"));
    assert_eq!(input.identity.species, vm.pet_render.generated_species);
    assert_eq!(input.identity.stage, vm.pet_render.stage);
    assert_eq!(input.mood, vm.pet_render.mood);
}

#[test]
fn pixel_variation_key_is_stable_without_exposing_seed_text() {
    let key_a = PixelVariationKey::from_seed("fixture-seed");
    let key_b = PixelVariationKey::from_seed("fixture-seed");
    let key_c = PixelVariationKey::from_seed("different-seed");

    assert_eq!(key_a, key_b);
    assert_ne!(key_a, key_c);
    assert!(!format!("{key_a:?}").contains("fixture-seed"));
}

#[test]
fn pixel_input_changes_for_live_identity_and_state_signals() {
    let now = datetime!(2026-07-08 12:00 UTC);
    let base = WatchViewModel::fixture();
    let mut other = base.clone();
    other.pet_render.generated_species = Species::Glitch;
    other.pet_render.stage = Stage::S4;
    other.pet_render.mood = Mood::Ecstatic;
    other.day_context.asleep = true;
    other.life_profile.calm_mode = true;
    other.life_profile.burst_level = 0.8;
    other.last_feed_pulse_at = Some(now - time::Duration::milliseconds(250));

    let base_input = PixelPetInput::from_watch_view_model(&base, now);
    let other_input = PixelPetInput::from_watch_view_model(&other, now);

    assert_ne!(base_input.identity.species, other_input.identity.species);
    assert_ne!(base_input.identity.stage, other_input.identity.stage);
    assert_ne!(base_input.mood, other_input.mood);
    assert!(!base_input.sleep.asleep);
    assert!(other_input.sleep.asleep);
    assert!(!base_input.pulse.active);
    assert!(other_input.pulse.active);
}

#[test]
fn pixel_frame_enforces_rgba_invariants() {
    let viewport = PixelViewport::companion_default();
    let frame = PixelFrame::transparent(viewport);

    assert_eq!(viewport.logical_width, 96);
    assert_eq!(viewport.logical_height, 96);
    assert_eq!(frame.width, 96);
    assert_eq!(frame.height, 96);
    assert_eq!(frame.pixels.len(), 96 * 96);
    assert_eq!(frame.opaque_pixel_count(), 0);
    assert_eq!(frame.opaque_bounds(), None);
    assert_eq!(frame.pixels[0], Rgba8 { r: 0, g: 0, b: 0, a: 0 });
}
