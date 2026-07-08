#![cfg(target_os = "macos")]

use crate::presentation::pixel::{pixel_runs, PixelFrame, Rgba8};
use crate::round::layout::RoundAperture;
use objc2::rc::Retained;
use objc2_app_kit::{NSBezierPath, NSColor};
use objc2_foundation::{NSPoint, NSRect, NSSize};

pub fn draw_pixel_frame(frame: &PixelFrame, bounds: NSRect, aperture: RoundAperture) {
    let dest_size = f64::from(aperture.radius * 2.0);
    let scale = dest_size / f64::from(frame.width.max(frame.height));
    let origin_x = f64::from(aperture.center_x - aperture.radius);
    let origin_y = f64::from(aperture.center_y - aperture.radius);
    let _ = bounds;

    unsafe {
        for run in pixel_runs(frame) {
            let x = origin_x + f64::from(run.x) * scale;
            let y = origin_y + f64::from(frame.height - run.y - 1) * scale;
            let rect = NSBezierPath::bezierPathWithRect(NSRect::new(
                NSPoint::new(x, y),
                NSSize::new(f64::from(run.width) * scale, scale),
            ));
            ns_color(run.color).setFill();
            rect.fill();
        }
    }
}

fn ns_color(color: Rgba8) -> Retained<NSColor> {
    unsafe {
        NSColor::colorWithSRGBRed_green_blue_alpha(
            f64::from(color.r) / 255.0,
            f64::from(color.g) / 255.0,
            f64::from(color.b) / 255.0,
            f64::from(color.a) / 255.0,
        )
    }
}
