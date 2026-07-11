#![cfg(target_os = "macos")]

use std::path::Path;
use std::time::Instant;
use std::{cell::RefCell, collections::HashMap};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBezierPath, NSBitmapImageFileType,
    NSBitmapImageRepPropertyKey, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSView,
};
use objc2_foundation::{
    NSDictionary, NSMutableAttributedString, NSPoint, NSRange, NSRect, NSSize, NSString,
};
use serde::Serialize;

use crate::error::{GlorpError, Result};

use super::artifacts::{self, FrameMetric};
use super::fixture::{
    canonical_fixture, resolve_frame, DecisionPrimitiveKind, DecisionResolvedPrimitive,
};

pub fn draw(view: &NSView, bounds: NSRect) {
    let started = Instant::now();
    let elapsed_ms = super::macos::elapsed_ms();
    let fixture = canonical_fixture();
    let frame = resolve_frame(&fixture, elapsed_ms);
    unsafe {
        let background = NSColor::colorWithSRGBRed_green_blue_alpha(0.025, 0.07, 0.09, 1.0);
        background.setFill();
        NSBezierPath::bezierPathWithRect(bounds).fill();
        let aperture = NSBezierPath::bezierPathWithOvalInRect(bounds);
        aperture.addClip();
    }
    let sx = bounds.size.width as f32 / 360.0;
    let sy = bounds.size.height as f32 / 360.0;
    for primitive in &frame.primitives {
        paint_primitive(primitive, bounds.size.height, sx, sy);
    }
    let metric = FrameMetric {
        frame_index: frame.frame_index,
        elapsed_ms,
        end_to_end_cpu_micros: started.elapsed().as_micros() as u64,
        requested_visible_frames: 1,
        completed_visible_frames: 1,
        submissions: 1,
        missed_deadlines: 0,
        primitive_count: frame.primitives.len() as u32,
        static_rebuilds: 1,
        atlas_misses: 0,
        upload_bytes: 0,
        static_upload_bytes: 0,
        dynamic_upload_bytes: 0,
        atlas_upload_bytes: 0,
        uniform_upload_bytes: 0,
        resource_generation: 0,
        draw_calls: 300,
    };
    super::macos::record_metric(metric);
    let _ = view;
}

fn paint_primitive(primitive: &DecisionResolvedPrimitive, height: f64, sx: f32, sy: f32) {
    let rect = NSRect::new(
        NSPoint::new(
            f64::from(primitive.bounds.x * sx),
            height - f64::from((primitive.bounds.y + primitive.bounds.height) * sy),
        ),
        NSSize::new(
            f64::from(primitive.bounds.width * sx),
            f64::from(primitive.bounds.height * sy),
        ),
    );
    unsafe {
        match primitive.kind {
            DecisionPrimitiveKind::Glyph => {
                let glyphs = [
                    "@", "#", "%", "&", "*", "+", "-", ".", "o", "O", "x", "X", "?", "�", "🫧",
                    "o\u{308}",
                ];
                let glyph = glyphs[usize::from(primitive.atlas_entry.unwrap_or(0)) % glyphs.len()];
                let font_size = rect.size.height.max(6.0);
                attributed_glyph(glyph, font_size, primitive.rgba)
                    .drawAtPoint(NSPoint::new(rect.origin.x, rect.origin.y));
            }
            DecisionPrimitiveKind::Rect => {
                fill_path(NSBezierPath::bezierPathWithRect(rect), primitive.rgba)
            }
            DecisionPrimitiveKind::Ellipse => {
                fill_path(NSBezierPath::bezierPathWithOvalInRect(rect), primitive.rgba)
            }
            DecisionPrimitiveKind::Arc => {
                let path = NSBezierPath::new();
                path.appendBezierPathWithArcWithCenter_radius_startAngle_endAngle_clockwise(
                    NSPoint::new(
                        rect.origin.x + rect.size.width / 2.0,
                        rect.origin.y + rect.size.height / 2.0,
                    ),
                    rect.size.width.min(rect.size.height) / 2.0,
                    20.0,
                    290.0,
                    false,
                );
                ns_color(primitive.rgba).setStroke();
                path.setLineWidth(2.0);
                path.stroke();
            }
        }
    }
}

unsafe fn fill_path(path: Retained<NSBezierPath>, rgba: [u8; 4]) {
    ns_color(rgba).setFill();
    path.fill();
}

fn ns_color(rgba: [u8; 4]) -> Retained<NSColor> {
    thread_local! {
        static COLORS: RefCell<HashMap<[u8; 4], Retained<NSColor>>> = RefCell::new(HashMap::new());
    }
    COLORS.with(|colors| {
        let mut colors = colors.borrow_mut();
        colors
            .entry(rgba)
            .or_insert_with(|| unsafe {
                NSColor::colorWithSRGBRed_green_blue_alpha(
                    f64::from(rgba[0]) / 255.0,
                    f64::from(rgba[1]) / 255.0,
                    f64::from(rgba[2]) / 255.0,
                    f64::from(rgba[3]) / 255.0,
                )
            })
            .clone()
    })
}

fn attributed_glyph(
    glyph: &str,
    size: f64,
    rgba: [u8; 4],
) -> Retained<objc2_foundation::NSAttributedString> {
    type Key = (String, i64, [u8; 4]);
    thread_local! {
        static GLYPHS: RefCell<HashMap<Key, Retained<objc2_foundation::NSAttributedString>>> =
            RefCell::new(HashMap::new());
    }
    let key = (glyph.to_string(), (size * 20.0).round() as i64, rgba);
    GLYPHS.with(|glyphs| {
        let mut glyphs = glyphs.borrow_mut();
        glyphs
            .entry(key)
            .or_insert_with(|| unsafe {
                let text = NSString::from_str(glyph);
                let font = NSFont::monospacedSystemFontOfSize_weight(size, 0.0);
                let mut attributed = NSMutableAttributedString::from_nsstring(&text);
                let range = NSRange::from(0..text.length());
                attributed.addAttribute_value_range(NSFontAttributeName, &font, range);
                attributed.addAttribute_value_range(
                    NSForegroundColorAttributeName,
                    &ns_color(rgba),
                    range,
                );
                Retained::into_super(attributed)
            })
            .clone()
    })
}

#[derive(Serialize)]
struct CaptureMetadata {
    schema_version: u16,
    logical_size: u16,
    physical_width: usize,
    physical_height: usize,
    frame_index: u64,
    orientation: &'static str,
    color_format: &'static str,
}

pub fn write_capture(
    view: &NSView,
    root: &Path,
    logical_size: u16,
    frame_index: u64,
) -> Result<()> {
    let captures = root.join("captures");
    std::fs::create_dir_all(&captures)?;
    let stem = format!("capture-{logical_size}-frame-{frame_index:06}");
    let png_path = captures.join(format!("{stem}.png"));
    let (physical_width, physical_height) = unsafe {
        view.displayIfNeeded();
        let bounds = view.bounds();
        let Some(bitmap) = view.bitmapImageRepForCachingDisplayInRect(bounds) else {
            return Err(GlorpError::Message(
                "renderer spike failed to allocate capture bitmap".into(),
            ));
        };
        let physical_width = bitmap.pixelsWide() as usize;
        let physical_height = bitmap.pixelsHigh() as usize;
        view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
        let properties: Retained<NSDictionary<NSBitmapImageRepPropertyKey, AnyObject>> =
            NSDictionary::new();
        let Some(data) =
            bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties)
        else {
            return Err(GlorpError::Message(
                "renderer spike failed to encode capture png".into(),
            ));
        };
        if !data.writeToFile_atomically(&NSString::from_str(&png_path.to_string_lossy()), true) {
            return Err(GlorpError::Message(
                "renderer spike failed to write capture png".into(),
            ));
        }
        (physical_width, physical_height)
    };
    artifacts::write_json(
        &captures.join(format!("{stem}.json")),
        &CaptureMetadata {
            schema_version: 1,
            logical_size,
            physical_width,
            physical_height,
            frame_index,
            orientation: "top-left",
            color_format: "rgba8-srgb-png",
        },
    )
}
