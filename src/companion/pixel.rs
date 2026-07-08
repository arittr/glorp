#![cfg(target_os = "macos")]

use crate::presentation::pixel::PixelFrame;
use crate::round::layout::RoundAperture;
use objc2::rc::Retained;
use objc2::ClassType;
use objc2_app_kit::{
    NSAlphaNonpremultipliedBitmapFormat, NSBitmapImageRep, NSCompositingOperation,
    NSDeviceRGBColorSpace, NSGraphicsContext, NSImage, NSImageInterpolation,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use std::ptr;

pub fn draw_pixel_frame(frame: &PixelFrame, _bounds: NSRect, aperture: RoundAperture) {
    let Some(bitmap) = bitmap_image_rep_for_frame(frame) else {
        return;
    };
    let image = ns_image_for_bitmap(frame, &bitmap);
    let Some(context) = (unsafe { NSGraphicsContext::currentContext() }) else {
        return;
    };
    let previous_interpolation = unsafe { context.imageInterpolation() };
    let previous_antialias = unsafe { context.shouldAntialias() };
    let aperture_rect = NSRect::new(
        NSPoint::new(
            f64::from(aperture.center_x - aperture.radius),
            f64::from(aperture.center_y - aperture.radius),
        ),
        NSSize::new(
            f64::from(aperture.radius * 2.0),
            f64::from(aperture.radius * 2.0),
        ),
    );
    let source_rect = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(f64::from(frame.width), f64::from(frame.height)),
    );

    unsafe {
        context.setImageInterpolation(NSImageInterpolation::None);
        context.setShouldAntialias(false);
        image.drawInRect_fromRect_operation_fraction(
            aperture_rect,
            source_rect,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        context.setImageInterpolation(previous_interpolation);
        context.setShouldAntialias(previous_antialias);
    }
}

fn bitmap_image_rep_for_frame(frame: &PixelFrame) -> Option<Retained<NSBitmapImageRep>> {
    let bytes = rgba_bytes_for_frame(frame);
    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bitmapFormat_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            ptr::null_mut(),
            frame.width as isize,
            frame.height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            NSAlphaNonpremultipliedBitmapFormat,
            frame.width as isize * 4,
            32,
        )
    }?;
    let bitmap_data = unsafe { bitmap.bitmapData() };
    if bitmap_data.is_null() {
        return None;
    }
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), bitmap_data.cast::<u8>(), bytes.len());
    }
    Some(bitmap)
}

fn ns_image_for_bitmap(frame: &PixelFrame, bitmap: &NSBitmapImageRep) -> Retained<NSImage> {
    let image = unsafe {
        NSImage::initWithSize(
            NSImage::alloc(),
            NSSize::new(f64::from(frame.width), f64::from(frame.height)),
        )
    };
    unsafe {
        image.addRepresentation(bitmap);
    }
    image
}

fn rgba_bytes_for_frame(frame: &PixelFrame) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(usize::from(frame.width) * usize::from(frame.height) * 4);
    for y in (0..frame.height).rev() {
        for x in 0..frame.width {
            let idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
            let color = frame.pixels[idx];
            bytes.extend_from_slice(&[color.r, color.g, color.b, color.a]);
        }
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::pixel::{PixelViewport, Rgba8};

    #[test]
    fn rgba_bytes_for_frame_packs_straight_rgba_bottom_row_first() {
        let mut frame =
            PixelFrame::transparent(PixelViewport { logical_width: 2, logical_height: 2 });
        let top_left = Rgba8 { r: 1, g: 2, b: 3, a: 4 };
        let top_right = Rgba8 { r: 5, g: 6, b: 7, a: 8 };
        let bottom_left = Rgba8 { r: 9, g: 10, b: 11, a: 12 };
        let bottom_right = Rgba8 { r: 13, g: 14, b: 15, a: 16 };
        frame.set_pixel(0, 0, top_left);
        frame.set_pixel(1, 0, top_right);
        frame.set_pixel(0, 1, bottom_left);
        frame.set_pixel(1, 1, bottom_right);

        let bytes = rgba_bytes_for_frame(&frame);

        assert_eq!(
            bytes,
            vec![9, 10, 11, 12, 13, 14, 15, 16, 1, 2, 3, 4, 5, 6, 7, 8,]
        );
    }

    #[test]
    fn bitmap_image_rep_round_trips_rgba_channels_and_top_row_orientation() {
        let mut frame =
            PixelFrame::transparent(PixelViewport { logical_width: 2, logical_height: 2 });
        let top_left = Rgba8::opaque(255, 0, 0);
        let top_right = Rgba8::opaque(0, 255, 0);
        let bottom_left = Rgba8::opaque(0, 0, 255);
        let bottom_right = Rgba8::opaque(255, 255, 0);
        frame.set_pixel(0, 0, top_left);
        frame.set_pixel(1, 0, top_right);
        frame.set_pixel(0, 1, bottom_left);
        frame.set_pixel(1, 1, bottom_right);

        let bitmap = bitmap_image_rep_for_frame(&frame).expect("bitmap image rep");

        assert_color_eq(unsafe { bitmap.colorAtX_y(0, 1) }.as_deref(), top_left);
        assert_color_eq(unsafe { bitmap.colorAtX_y(1, 1) }.as_deref(), top_right);
        assert_color_eq(unsafe { bitmap.colorAtX_y(0, 0) }.as_deref(), bottom_left);
        assert_color_eq(unsafe { bitmap.colorAtX_y(1, 0) }.as_deref(), bottom_right);
    }

    fn assert_color_eq(color: Option<&objc2_app_kit::NSColor>, expected: Rgba8) {
        let color = color.expect("color channel");
        let actual = unsafe {
            Rgba8 {
                r: (color.redComponent() * 255.0).round() as u8,
                g: (color.greenComponent() * 255.0).round() as u8,
                b: (color.blueComponent() * 255.0).round() as u8,
                a: (color.alphaComponent() * 255.0).round() as u8,
            }
        };
        assert_eq!(actual, expected);
    }
}
