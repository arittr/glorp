use super::frame::{PixelFrame, Rgba8};

pub fn fill_rect(frame: &mut PixelFrame, x0: i16, y0: i16, width: i16, height: i16, color: Rgba8) {
    for y in y0..y0 + height {
        for x in x0..x0 + width {
            frame.set_pixel(x, y, color);
        }
    }
}

pub fn fill_ellipse(frame: &mut PixelFrame, cx: i16, cy: i16, rx: i16, ry: i16, color: Rgba8) {
    let rx2 = i32::from(rx.max(1)).pow(2);
    let ry2 = i32::from(ry.max(1)).pow(2);
    let limit = rx2 * ry2;
    for y in cy - ry..=cy + ry {
        for x in cx - rx..=cx + rx {
            let dx = i32::from(x - cx);
            let dy = i32::from(y - cy);
            if dx * dx * ry2 + dy * dy * rx2 <= limit {
                frame.set_pixel(x, y, color);
            }
        }
    }
}

pub fn fill_circle(frame: &mut PixelFrame, cx: i16, cy: i16, radius: i16, color: Rgba8) {
    fill_ellipse(frame, cx, cy, radius, radius, color);
}

pub fn alpha_blend_pixel(frame: &mut PixelFrame, x: i16, y: i16, color: Rgba8) {
    if color.a == 255 {
        frame.set_pixel(x, y, color);
        return;
    }
    if x < 0 || y < 0 || x as u16 >= frame.width || y as u16 >= frame.height {
        return;
    }
    let idx = usize::from(y as u16) * usize::from(frame.width) + usize::from(x as u16);
    let dst = frame.pixels[idx];
    let src_a = f32::from(color.a) / 255.0;
    let dst_a = f32::from(dst.a) / 255.0;
    let out_a = src_a + dst_a * (1.0 - src_a);
    if out_a <= f32::EPSILON {
        frame.pixels[idx] = Rgba8::TRANSPARENT;
        return;
    }
    let src_w = src_a / out_a;
    let dst_w = (dst_a * (1.0 - src_a)) / out_a;
    frame.pixels[idx] = Rgba8 {
        r: (f32::from(color.r) * src_w + f32::from(dst.r) * dst_w).round() as u8,
        g: (f32::from(color.g) * src_w + f32::from(dst.g) * dst_w).round() as u8,
        b: (f32::from(color.b) * src_w + f32::from(dst.b) * dst_w).round() as u8,
        a: (out_a * 255.0).round() as u8,
    };
}
