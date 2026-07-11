use super::fixture::{
    DecisionAtlas, DecisionPrimitiveKind, DecisionResolvedFrame, DecisionResolvedPrimitive,
};

pub const SOFTWARE_LOGICAL_EXTENT: f32 = 360.0;
pub const SOFTWARE_BACKGROUND_RGBA: [u8; 4] = [6, 14, 22, 255];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoftwareRasterStats {
    pub primitive_count: u32,
    pub rasterized_pixels: u64,
    pub atlas_misses: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoftwareFramebuffer {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl SoftwareFramebuffer {
    pub fn new(width: u32, height: u32) -> Result<Self, String> {
        if width == 0 || height == 0 {
            return Err("software framebuffer dimensions must be nonzero".to_string());
        }
        let pixel_count = usize::try_from(width)
            .ok()
            .and_then(|width| {
                usize::try_from(height)
                    .ok()
                    .and_then(|height| width.checked_mul(height))
            })
            .ok_or_else(|| "software framebuffer dimensions overflow".to_string())?;
        let byte_count = pixel_count
            .checked_mul(4)
            .ok_or_else(|| "software framebuffer byte length overflows".to_string())?;
        Ok(Self {
            width,
            height,
            pixels: vec![0; byte_count],
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let offset = pixel_offset(self.width, self.height, x, y)?;
        Some([
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ])
    }

    pub fn render(
        &mut self,
        frame: &DecisionResolvedFrame,
        atlas: &DecisionAtlas,
    ) -> SoftwareRasterStats {
        self.clear(SOFTWARE_BACKGROUND_RGBA);
        let mut primitives = frame.primitives.iter().collect::<Vec<_>>();
        primitives.sort_by_key(|primitive| (primitive.depth_band, primitive.id));
        let mut stats = SoftwareRasterStats {
            primitive_count: primitives.len() as u32,
            rasterized_pixels: 0,
            atlas_misses: 0,
        };
        for primitive in primitives {
            self.raster_primitive(primitive, atlas, &mut stats);
        }
        stats
    }

    fn clear(&mut self, rgba: [u8; 4]) {
        let premultiplied = premultiply(rgba);
        for pixel in self.pixels.chunks_exact_mut(4) {
            pixel.copy_from_slice(&premultiplied);
        }
    }

    fn raster_primitive(
        &mut self,
        primitive: &DecisionResolvedPrimitive,
        atlas: &DecisionAtlas,
        stats: &mut SoftwareRasterStats,
    ) {
        let scale_x = self.width as f32 / SOFTWARE_LOGICAL_EXTENT;
        let scale_y = self.height as f32 / SOFTWARE_LOGICAL_EXTENT;
        let x = primitive.bounds.x * scale_x;
        let y = primitive.bounds.y * scale_y;
        let width = primitive.bounds.width * scale_x;
        let height = primitive.bounds.height * scale_y;
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return;
        }

        let Some((min_x, max_x)) = clipped_axis_range(x, width, self.width) else {
            return;
        };
        let Some((min_y, max_y)) = clipped_axis_range(y, height, self.height) else {
            return;
        };
        let atlas_entry = primitive
            .atlas_entry
            .and_then(|index| atlas.entries.get(usize::from(index)));
        if primitive.kind == DecisionPrimitiveKind::Glyph && atlas_entry.is_none() {
            stats.atlas_misses = stats.atlas_misses.saturating_add(1);
            return;
        }

        for pixel_y in min_y..max_y {
            for pixel_x in min_x..max_x {
                if !inside_aperture(pixel_x, pixel_y, self.width, self.height) {
                    continue;
                }
                let local_x = (pixel_x as f32 + 0.5 - x) / width;
                let local_y = (pixel_y as f32 + 0.5 - y) / height;
                if !(0.0..1.0).contains(&local_x) || !(0.0..1.0).contains(&local_y) {
                    continue;
                }
                let coverage = match primitive.kind {
                    DecisionPrimitiveKind::Glyph => atlas_alpha(
                        atlas,
                        atlas_entry.expect("glyph atlas entry checked above"),
                        local_x,
                        local_y,
                    ),
                    DecisionPrimitiveKind::Rect => 255,
                    DecisionPrimitiveKind::Ellipse => ellipse_coverage(local_x, local_y),
                    DecisionPrimitiveKind::Arc => arc_coverage(local_x, local_y),
                };
                if coverage == 0 {
                    continue;
                }
                let source = color_with_coverage(primitive.rgba, coverage);
                self.blend_pixel(pixel_x, pixel_y, source);
                stats.rasterized_pixels = stats.rasterized_pixels.saturating_add(1);
            }
        }
    }

    fn blend_pixel(&mut self, x: u32, y: u32, source: [u8; 4]) {
        let Some(offset) = pixel_offset(self.width, self.height, x, y) else {
            return;
        };
        let destination = [
            self.pixels[offset],
            self.pixels[offset + 1],
            self.pixels[offset + 2],
            self.pixels[offset + 3],
        ];
        self.pixels[offset..offset + 4]
            .copy_from_slice(&source_over(premultiply(source), destination));
    }
}

fn clipped_axis_range(origin: f32, length: f32, limit: u32) -> Option<(u32, u32)> {
    let start = origin.floor().max(0.0).min(limit as f32) as u32;
    let end = (origin + length).ceil().max(0.0).min(limit as f32) as u32;
    (start < end).then_some((start, end))
}

fn inside_aperture(x: u32, y: u32, width: u32, height: u32) -> bool {
    let nx = (x as f32 + 0.5) / width as f32 * 2.0 - 1.0;
    let ny = (y as f32 + 0.5) / height as f32 * 2.0 - 1.0;
    nx * nx + ny * ny <= 1.0
}

fn atlas_alpha(
    atlas: &DecisionAtlas,
    entry: &super::fixture::DecisionAtlasEntry,
    local_x: f32,
    local_y: f32,
) -> u8 {
    let entry_width = u32::from(entry.rect[2]);
    let entry_height = u32::from(entry.rect[3]);
    if entry_width == 0 || entry_height == 0 {
        return 0;
    }
    let local_pixel_x = ((local_x * entry_width as f32).floor() as u32).min(entry_width - 1);
    let local_pixel_y = ((local_y * entry_height as f32).floor() as u32).min(entry_height - 1);
    let atlas_x = u32::from(entry.rect[0]).saturating_add(local_pixel_x);
    let atlas_y = u32::from(entry.rect[1]).saturating_add(local_pixel_y);
    let Some(offset) = pixel_offset(
        u32::from(atlas.width),
        u32::from(atlas.height),
        atlas_x,
        atlas_y,
    ) else {
        return 0;
    };
    atlas.rgba.get(offset + 3).copied().unwrap_or(0)
}

fn ellipse_coverage(local_x: f32, local_y: f32) -> u8 {
    let x = local_x * 2.0 - 1.0;
    let y = local_y * 2.0 - 1.0;
    u8::from(x * x + y * y <= 1.0) * 255
}

fn arc_coverage(local_x: f32, local_y: f32) -> u8 {
    let x = local_x * 2.0 - 1.0;
    let y = local_y * 2.0 - 1.0;
    let radius = (x * x + y * y).sqrt();
    let angle = y.atan2(x);
    u8::from((0.62..=1.0).contains(&radius) && !(-0.35..0.35).contains(&angle)) * 255
}

fn color_with_coverage(mut rgba: [u8; 4], coverage: u8) -> [u8; 4] {
    rgba[3] = mul_unorm8(rgba[3], coverage);
    rgba
}

fn premultiply(rgba: [u8; 4]) -> [u8; 4] {
    [
        mul_unorm8(rgba[0], rgba[3]),
        mul_unorm8(rgba[1], rgba[3]),
        mul_unorm8(rgba[2], rgba[3]),
        rgba[3],
    ]
}

fn source_over(source: [u8; 4], destination: [u8; 4]) -> [u8; 4] {
    let inverse_alpha = 255_u8.saturating_sub(source[3]);
    [
        source[0].saturating_add(mul_unorm8(destination[0], inverse_alpha)),
        source[1].saturating_add(mul_unorm8(destination[1], inverse_alpha)),
        source[2].saturating_add(mul_unorm8(destination[2], inverse_alpha)),
        source[3].saturating_add(mul_unorm8(destination[3], inverse_alpha)),
    ]
}

fn mul_unorm8(left: u8, right: u8) -> u8 {
    let product = u16::from(left) * u16::from(right);
    ((product + 127) / 255) as u8
}

fn pixel_offset(width: u32, height: u32, x: u32, y: u32) -> Option<usize> {
    if x >= width || y >= height {
        return None;
    }
    usize::try_from(y)
        .ok()?
        .checked_mul(usize::try_from(width).ok()?)?
        .checked_add(usize::try_from(x).ok()?)?
        .checked_mul(4)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer_spike::fixture::{
        canonical_atlas, canonical_fixture, resolve_frame, DecisionPrimitiveId, DecisionRect,
    };

    #[test]
    fn dimensions_have_exact_rgba_lengths() {
        let retina_360 = SoftwareFramebuffer::new(720, 720).unwrap();
        assert_eq!(retina_360.pixels().len(), 720 * 720 * 4);
        let retina_720 = SoftwareFramebuffer::new(1440, 1440).unwrap();
        assert_eq!(retina_720.pixels().len(), 1440 * 1440 * 4);
        assert!(SoftwareFramebuffer::new(0, 720).is_err());
    }

    #[test]
    fn premultiplied_source_over_handles_known_vectors() {
        assert_eq!(premultiply([200, 100, 50, 0]), [0, 0, 0, 0]);
        assert_eq!(premultiply([200, 100, 50, 255]), [200, 100, 50, 255]);
        assert_eq!(premultiply([200, 100, 50, 128]), [100, 50, 25, 128]);
        assert_eq!(
            source_over(premultiply([255, 0, 0, 128]), premultiply([0, 0, 255, 255])),
            [128, 0, 127, 255]
        );
    }

    #[test]
    fn clipping_rejects_outside_pixels_and_keeps_partial_pixels() {
        assert_eq!(clipped_axis_range(-5.0, 3.0, 10), None);
        assert_eq!(clipped_axis_range(-2.0, 5.0, 10), Some((0, 3)));
        assert_eq!(clipped_axis_range(8.0, 5.0, 10), Some((8, 10)));
        assert_eq!(pixel_offset(4, 4, 4, 0), None);
    }

    #[test]
    fn atlas_blit_uses_nearest_alpha_and_tint() {
        let atlas = canonical_atlas();
        let entry = &atlas.entries[0];
        assert_eq!(atlas_alpha(&atlas, entry, 0.01, 0.01), 0);
        assert!(atlas_alpha(&atlas, entry, 0.25, 0.25) > 0);
        assert_eq!(
            color_with_coverage([20, 40, 60, 128], 255),
            [20, 40, 60, 128]
        );
        assert_eq!(color_with_coverage([20, 40, 60, 128], 0), [20, 40, 60, 0]);
    }

    #[test]
    fn software_shapes_match_shader_inclusion_rules() {
        assert_eq!(ellipse_coverage(0.5, 0.5), 255);
        assert_eq!(ellipse_coverage(0.0, 0.0), 0);
        assert_eq!(arc_coverage(0.5, 0.5), 0);
        assert_eq!(arc_coverage(0.5, 0.0), 255);
        assert_eq!(arc_coverage(1.0, 0.5), 0);
    }

    #[test]
    fn aperture_uses_top_left_pixel_coordinates() {
        assert!(!inside_aperture(0, 0, 100, 100));
        assert!(inside_aperture(50, 50, 100, 100));
        assert!(!inside_aperture(99, 99, 100, 100));
    }

    #[test]
    fn logical_y_is_not_mirrored_in_raw_top_left_pixels() {
        let frame = DecisionResolvedFrame {
            frame_index: 0,
            elapsed_ms: 0,
            primitives: vec![DecisionResolvedPrimitive {
                id: DecisionPrimitiveId(1),
                kind: DecisionPrimitiveKind::Rect,
                atlas_entry: None,
                bounds: DecisionRect {
                    x: 170.0,
                    y: 40.0,
                    width: 20.0,
                    height: 20.0,
                },
                rgba: [255, 0, 0, 255],
                depth_band: 0,
            }],
            changed_primitive_ids: Vec::new(),
        };
        let mut framebuffer = SoftwareFramebuffer::new(360, 360).unwrap();
        framebuffer.render(&frame, &canonical_atlas());
        assert_eq!(framebuffer.pixel(180, 50), Some([255, 0, 0, 255]));
        assert_eq!(
            framebuffer.pixel(180, 310),
            Some(premultiply(SOFTWARE_BACKGROUND_RGBA))
        );
    }

    #[test]
    fn canonical_frames_render_all_primitives_with_zero_atlas_misses() {
        let fixture = canonical_fixture();
        let atlas = canonical_atlas();
        let mut framebuffer = SoftwareFramebuffer::new(720, 720).unwrap();
        for elapsed_ms in [0, 250, 1_000, 5_000] {
            let frame = resolve_frame(&fixture, elapsed_ms);
            let stats = framebuffer.render(&frame, &atlas);
            assert_eq!(stats.primitive_count, 300);
            assert_eq!(stats.atlas_misses, 0);
            assert!(stats.rasterized_pixels > 0);
            assert!(framebuffer
                .pixels()
                .chunks_exact(4)
                .any(|pixel| pixel != premultiply(SOFTWARE_BACKGROUND_RGBA)));
        }
    }

    #[test]
    fn primitive_order_is_depth_then_id() {
        let mut frame = DecisionResolvedFrame {
            frame_index: 0,
            elapsed_ms: 0,
            primitives: vec![
                DecisionResolvedPrimitive {
                    id: DecisionPrimitiveId(2),
                    kind: DecisionPrimitiveKind::Rect,
                    atlas_entry: None,
                    bounds: DecisionRect {
                        x: 170.0,
                        y: 170.0,
                        width: 20.0,
                        height: 20.0,
                    },
                    rgba: [255, 0, 0, 255],
                    depth_band: 2,
                },
                DecisionResolvedPrimitive {
                    id: DecisionPrimitiveId(1),
                    kind: DecisionPrimitiveKind::Rect,
                    atlas_entry: None,
                    bounds: DecisionRect {
                        x: 170.0,
                        y: 170.0,
                        width: 20.0,
                        height: 20.0,
                    },
                    rgba: [0, 255, 0, 255],
                    depth_band: 0,
                },
            ],
            changed_primitive_ids: Vec::new(),
        };
        frame.primitives.reverse();
        let mut framebuffer = SoftwareFramebuffer::new(360, 360).unwrap();
        framebuffer.render(&frame, &canonical_atlas());
        let offset = pixel_offset(360, 360, 180, 180).unwrap();
        assert_eq!(&framebuffer.pixels()[offset..offset + 4], &[255, 0, 0, 255]);
    }
}
