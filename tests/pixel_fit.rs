use glorp::presentation::pixel::{PixelBounds, PixelViewport};
use glorp::round::hud::companion_hud_text;
use glorp::round::pixel_fit::{pixel_companion_fit, PixelTargetGeometry};

fn fit_for(size: u16) -> glorp::round::pixel_fit::PixelCompanionFit {
    let hud = companion_hud_text(205_700_000.0, Some(9.99), 9_900_000.0);
    pixel_companion_fit(
        PixelTargetGeometry { width: size, height: size },
        PixelViewport::companion_default(),
        &hud,
    )
}

#[test]
fn pixel_fit_does_not_use_the_entire_aperture() {
    let fit = fit_for(360);

    assert!(fit.image_rect.width < fit.aperture.radius * 2.0);
    assert!(fit.image_rect.height < fit.aperture.radius * 2.0);
    assert!(fit.image_rect.y < fit.hud_safe_zone.y);
}

#[test]
fn body_bounds_do_not_overlap_hud_safe_zone_for_target_geometries() {
    let body = PixelBounds {
        min_x: 26,
        min_y: 20,
        max_x: 70,
        max_y: 67,
    };
    for size in [260_u16, 360, 480, 900] {
        let fit = fit_for(size);
        assert!(
            !fit.logical_bounds_overlap_hud(body),
            "body bounds overlapped HUD safe zone for {size}x{size}: {fit:?}"
        );
    }
}

#[test]
fn fit_names_production_helper_for_preview_contracts() {
    let fit = fit_for(360);

    assert_eq!(fit.producer, "round::pixel_fit::pixel_companion_fit");
}
