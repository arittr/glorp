use glorp::game::{evolution::Stage, metabolism::Mood};
use glorp::pet::generation::Species;
use glorp::presentation::pixel::{
    PixelBounds, PixelFrame, PixelPetInput, PixelVariationKey, PixelViewport, Rgba8,
};
use glorp::tui::view_model::{SourceUsageView, WatchViewModel};
use std::sync::{Mutex, OnceLock};
use time::macros::datetime;

fn catch_unwind_silently<F, T>(f: F) -> Result<T, Box<dyn std::any::Any + Send>>
where
    F: FnOnce() -> T + std::panic::UnwindSafe,
{
    static PANIC_HOOK_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _lock = PANIC_HOOK_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap();
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));

    let result = std::panic::catch_unwind(f);
    std::panic::set_hook(prev_hook);
    result
}

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
    assert!(!other_input.pulse.active);
    assert_eq!(other_input.pulse.age_ms, 250);
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

#[test]
fn pixel_frame_helper_methods_reject_malformed_storage() {
    let malformed = PixelFrame {
        width: 2,
        height: 2,
        pixels: vec![Rgba8::TRANSPARENT; 3],
    };
    let other = PixelFrame::transparent(PixelViewport { logical_width: 2, logical_height: 2 });

    let opaque_count = catch_unwind_silently(|| malformed.opaque_pixel_count());
    assert!(opaque_count.is_err());

    let opaque_bounds = catch_unwind_silently(|| malformed.opaque_bounds());
    assert!(opaque_bounds.is_err());

    let changed_count = catch_unwind_silently(|| malformed.changed_pixel_count(&other));
    assert!(changed_count.is_err());

    let mut malformed_for_set = malformed.clone();
    let set_pixel = catch_unwind_silently(std::panic::AssertUnwindSafe(|| {
        malformed_for_set.set_pixel(1, 1, Rgba8::opaque(0xff, 0x00, 0x00));
    }));
    assert!(set_pixel.is_err());
}

#[test]
fn pixel_frame_reports_bounds_and_changed_pixels_for_sparse_updates() {
    let viewport = PixelViewport { logical_width: 4, logical_height: 3 };
    let base = PixelFrame::transparent(viewport);
    let mut updated = base.clone();

    updated.set_pixel(1, 0, Rgba8::opaque(0x11, 0x22, 0x33));
    updated.set_pixel(3, 2, Rgba8::opaque(0xaa, 0xbb, 0xcc));
    updated.set_pixel(-1, 0, Rgba8::opaque(0xff, 0x00, 0x00));
    updated.set_pixel(4, 2, Rgba8::opaque(0x00, 0xff, 0x00));

    assert_eq!(base.changed_pixel_count(&updated), 2);
    assert_eq!(updated.changed_pixel_count(&base), 2);
    assert_eq!(
        updated.opaque_bounds(),
        Some(PixelBounds { min_x: 1, min_y: 0, max_x: 3, max_y: 2 })
    );
}
