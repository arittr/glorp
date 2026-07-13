//! Canonical GPU readback for the retained companion renderer.
//!
//! This ports the proven wgpu capture sequence (render a frozen scene into an
//! off-screen intermediate, copy it to a mappable staging buffer, map, and read
//! it back CPU-side) behind production-owned types. The readback is re-derived
//! here, never imported from the renderer spike, so the paired-review parity
//! evidence rests entirely on production code.
//!
//! The row-normalization step ([`normalize_readback_rows`]) is pure and
//! unit-tested: it drops the GPU's 256-byte row padding, swizzles BGRA surfaces
//! into canonical top-left RGBA, and rejects a mapped buffer that is too short
//! for the frame's row layout.

use std::sync::mpsc;
use std::time::Duration;

use super::host::RetainedHost;
use super::presentation::RetainedFailureCategory;
use super::{prepare_gpu_frame, RetainedChrome};
use crate::companion::paired_review::{PairedReviewFrame, RendererIdentitySource};
use crate::round::draw::RoundColor;

/// Channel order of a mapped readback row. Metal surfaces are commonly BGRA;
/// the canonical artifact is always RGBA, so a BGRA source is swizzled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PixelOrder {
    Rgba,
    Bgra,
}

/// A canonical, straight-alpha, top-left-origin RGBA8 frame read back off the
/// GPU. `frame_id`/`resource_generation` are stamped from the
/// [`PairedReviewFrame`](super::super::paired_review::PairedReviewFrame) the
/// capture repainted, so an artifact correlates 1:1 with its review row.
#[derive(Debug, Clone)]
pub(crate) struct CanonicalRgbaFrame {
    frame_id: u64,
    resource_generation: u64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl CanonicalRgbaFrame {
    pub(crate) fn frame_id(&self) -> u64 {
        self.frame_id
    }

    pub(crate) fn resource_generation(&self) -> u64 {
        self.resource_generation
    }

    pub(crate) fn width(&self) -> u32 {
        self.width
    }

    pub(crate) fn height(&self) -> u32 {
        self.height
    }

    pub(crate) fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// The physical-size row layout a readback copies through: the tightly packed
/// `width * 4` span, the 256-byte-aligned padded stride the GPU requires for
/// `copy_texture_to_buffer`, and the channel order of the source surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReadbackMetadata {
    pub(super) physical_width: u32,
    pub(super) physical_height: u32,
    pub(super) unpadded_bytes_per_row: u32,
    pub(super) aligned_bytes_per_row: u32,
    pub(super) pixel_order: PixelOrder,
}

impl ReadbackMetadata {
    /// Derives the row layout for a physical-size surface of `format`.
    pub(super) fn for_surface(width: u32, height: u32, format: wgpu::TextureFormat) -> Self {
        Self {
            physical_width: width,
            physical_height: height,
            unpadded_bytes_per_row: width.saturating_mul(4),
            aligned_bytes_per_row: aligned_bytes_per_row(width),
            pixel_order: pixel_order_for_format(format),
        }
    }
}

/// The mappable staging-buffer size for a physical-size readback, independent of
/// the surface format (the padded row stride depends only on width). The host
/// sizes its persistent staging buffer with this.
pub(super) fn staging_buffer_size(width: u32, height: u32) -> u64 {
    u64::from(aligned_bytes_per_row(width)) * u64::from(height)
}

/// Rounds a tightly packed `width * 4` row up to wgpu's copy-row alignment.
pub(super) fn aligned_bytes_per_row(width: u32) -> u32 {
    let raw = width.saturating_mul(4);
    raw.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        .saturating_mul(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
}

/// Maps a surface texture format onto its canonical readback channel order.
fn pixel_order_for_format(format: wgpu::TextureFormat) -> PixelOrder {
    if matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    ) {
        PixelOrder::Bgra
    } else {
        PixelOrder::Rgba
    }
}

/// Checked row geometry for the forward-only companion scene capture seam.
///
/// The scene renderer has one readback intermediate contract:
/// `Bgra8UnormSrgb`, copied in top-left row order. Keeping the format and order
/// closed here prevents a newly introduced surface format from silently taking
/// the legacy readback path's default-RGBA interpretation.
#[allow(dead_code)] // Consumed by the scene renderer in the readback activation slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneReadbackLayout {
    width: u32,
    height: u32,
    unpadded_bytes_per_row: u32,
    aligned_bytes_per_row: u32,
    buffer_size: u64,
    format: wgpu::TextureFormat,
    pixel_order: PixelOrder,
}

/// Fail-closed errors from deriving or normalizing the scene readback layout.
#[allow(dead_code)] // Consumed by the scene renderer in the readback activation slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneReadbackError {
    EmptyExtent,
    UnsupportedFormat,
    RowBytesOverflow,
    AlignedStrideOverflow,
    BufferSizeOverflow,
    BufferLimitExceeded { required: u64, maximum: u64 },
    InvalidLayout,
    SourceBufferTooShort,
}

#[allow(dead_code)] // Consumed by the scene renderer in the readback activation slice.
impl SceneReadbackLayout {
    /// Derives a copy-compatible layout, optionally checking the result against
    /// the active device's maximum buffer size.
    pub(super) fn checked(
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        max_buffer_size: Option<u64>,
    ) -> Result<Self, SceneReadbackError> {
        if width == 0 || height == 0 {
            return Err(SceneReadbackError::EmptyExtent);
        }
        if format != wgpu::TextureFormat::Bgra8UnormSrgb {
            return Err(SceneReadbackError::UnsupportedFormat);
        }

        let unpadded_bytes_per_row = width
            .checked_mul(4)
            .ok_or(SceneReadbackError::RowBytesOverflow)?;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let remainder = unpadded_bytes_per_row % alignment;
        let aligned_bytes_per_row = if remainder == 0 {
            unpadded_bytes_per_row
        } else {
            unpadded_bytes_per_row
                .checked_add(alignment - remainder)
                .ok_or(SceneReadbackError::AlignedStrideOverflow)?
        };
        let buffer_size = u64::from(aligned_bytes_per_row)
            .checked_mul(u64::from(height))
            .ok_or(SceneReadbackError::BufferSizeOverflow)?;
        if let Some(maximum) = max_buffer_size {
            if buffer_size > maximum {
                return Err(SceneReadbackError::BufferLimitExceeded {
                    required: buffer_size,
                    maximum,
                });
            }
        }

        Ok(Self {
            width,
            height,
            unpadded_bytes_per_row,
            aligned_bytes_per_row,
            buffer_size,
            format,
            pixel_order: PixelOrder::Bgra,
        })
    }

    pub(super) const fn width(self) -> u32 {
        self.width
    }

    pub(super) const fn height(self) -> u32 {
        self.height
    }

    pub(super) const fn unpadded_bytes_per_row(self) -> u32 {
        self.unpadded_bytes_per_row
    }

    pub(super) const fn aligned_bytes_per_row(self) -> u32 {
        self.aligned_bytes_per_row
    }

    pub(super) const fn buffer_size(self) -> u64 {
        self.buffer_size
    }
}

/// Removes copy-row padding and converts the scene intermediate into canonical
/// top-left, straight-alpha RGBA8 sRGB pixels.
///
/// An sRGB render target stores an sRGB encoding of the premultiplied **linear**
/// shader result. Consequently unpremultiplication must decode each stored RGB
/// byte to linear, divide by alpha there, then encode back to sRGB. Dividing the
/// stored bytes directly would brighten translucent glyph edges incorrectly.
#[allow(dead_code)] // Consumed by the scene renderer in the readback activation slice.
pub(super) fn normalize_scene_readback(
    source: &[u8],
    layout: SceneReadbackLayout,
) -> Result<Vec<u8>, SceneReadbackError> {
    let canonical = SceneReadbackLayout::checked(layout.width, layout.height, layout.format, None)?;
    if layout != canonical {
        return Err(SceneReadbackError::InvalidLayout);
    }

    let required =
        usize::try_from(layout.buffer_size).map_err(|_| SceneReadbackError::BufferSizeOverflow)?;
    if source.len() < required {
        return Err(SceneReadbackError::SourceBufferTooShort);
    }

    let output_size = u64::from(layout.unpadded_bytes_per_row)
        .checked_mul(u64::from(layout.height))
        .ok_or(SceneReadbackError::BufferSizeOverflow)?;
    let output_size =
        usize::try_from(output_size).map_err(|_| SceneReadbackError::BufferSizeOverflow)?;
    let stride = usize::try_from(layout.aligned_bytes_per_row)
        .map_err(|_| SceneReadbackError::AlignedStrideOverflow)?;
    let row_bytes = usize::try_from(layout.unpadded_bytes_per_row)
        .map_err(|_| SceneReadbackError::RowBytesOverflow)?;
    let mut rgba = vec![0_u8; output_size];

    for row in
        0..usize::try_from(layout.height).map_err(|_| SceneReadbackError::BufferSizeOverflow)?
    {
        let source_start = row
            .checked_mul(stride)
            .ok_or(SceneReadbackError::InvalidLayout)?;
        let source_end = source_start
            .checked_add(row_bytes)
            .ok_or(SceneReadbackError::InvalidLayout)?;
        let source_row = source
            .get(source_start..source_end)
            .ok_or(SceneReadbackError::SourceBufferTooShort)?;
        let destination_start = row
            .checked_mul(row_bytes)
            .ok_or(SceneReadbackError::InvalidLayout)?;
        let destination_end = destination_start
            .checked_add(row_bytes)
            .ok_or(SceneReadbackError::InvalidLayout)?;
        let destination_row = rgba
            .get_mut(destination_start..destination_end)
            .ok_or(SceneReadbackError::InvalidLayout)?;
        for (bgra, destination) in source_row
            .chunks_exact(4)
            .zip(destination_row.chunks_exact_mut(4))
        {
            let alpha_byte = bgra[3];
            if alpha_byte == 0 {
                destination.copy_from_slice(&[0, 0, 0, 0]);
                continue;
            }

            let alpha = f32::from(alpha_byte) / 255.0;
            destination[0] = straight_scene_channel(bgra[2], alpha);
            destination[1] = straight_scene_channel(bgra[1], alpha);
            destination[2] = straight_scene_channel(bgra[0], alpha);
            destination[3] = alpha_byte;
        }
    }

    Ok(rgba)
}

#[allow(dead_code)] // Used by normalize_scene_readback when scene capture is activated.
fn straight_scene_channel(stored_srgb: u8, alpha: f32) -> u8 {
    let premultiplied_linear = super::render::scene_srgb_to_linear(f32::from(stored_srgb) / 255.0);
    let straight_linear = (premultiplied_linear / alpha).clamp(0.0, 1.0);
    (super::render::scene_linear_to_srgb(straight_linear).clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Normalizes a mapped GPU readback buffer into canonical top-left RGBA8.
///
/// The mapped buffer stores each texture row at `bytes_per_row` (256-byte
/// aligned) stride; only the leading `width * 4` bytes of each row are real
/// pixels. This copies those real bytes in top-left order, swaps B/R when the
/// source is BGRA, and returns [`RetainedFailureCategory::CaptureBufferTooShort`]
/// when the mapped buffer cannot even hold the last row's real span.
///
/// This is a pure **row-layout** step: it drops padding and swizzles channel
/// order but never touches color. The premultiplied-sRGB → straight-sRGB
/// unpremultiply is a separate color seam applied by [`RetainedCaptureTarget`]
/// via [`super::parity::canonical_png_rgba`] once the rows are assembled, so the
/// two concerns (layout vs color convention) stay independently testable.
pub(super) fn normalize_readback_rows(
    source: &[u8],
    width: u32,
    height: u32,
    bytes_per_row: u32,
    order: PixelOrder,
) -> Result<Vec<u8>, RetainedFailureCategory> {
    let unpadded = (width as usize) * 4;
    let stride = bytes_per_row as usize;
    let rows = height as usize;

    // The buffer must reach the end of the last row's real span. Earlier rows
    // are addressed at full `stride`; only the final row needs just `unpadded`.
    let required = stride
        .saturating_mul(rows.saturating_sub(1))
        .saturating_add(unpadded);
    if source.len() < required {
        return Err(RetainedFailureCategory::CaptureBufferTooShort);
    }

    let mut rgba = vec![0_u8; unpadded * rows];
    for row in 0..rows {
        let source_start = row * stride;
        let destination_start = row * unpadded;
        rgba[destination_start..destination_start + unpadded]
            .copy_from_slice(&source[source_start..source_start + unpadded]);
    }

    if matches!(order, PixelOrder::Bgra) {
        for pixel in rgba.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
    }

    Ok(rgba)
}

/// A live GPU readback target for the retained renderer. It repaints a frozen
/// paired-review frame into the host's persistent off-screen intermediate and
/// reads it back as a [`CanonicalRgbaFrame`]. The intermediate, staging buffer,
/// and instance ring are all host-owned and reused; a same-size capture
/// allocates nothing.
pub(super) struct RetainedCaptureTarget<'host> {
    host: &'host mut RetainedHost,
}

impl<'host> RetainedCaptureTarget<'host> {
    /// Borrows the active host whose device/queue/pipelines/atlas the readback
    /// reuses so the capture rasterizes with the identical pipeline as `render`.
    pub(super) fn new(host: &'host mut RetainedHost) -> Self {
        Self { host }
    }

    /// Renders `frame`'s frozen Smooth scene into a physical-size sRGB
    /// intermediate and reads it back as a [`CanonicalRgbaFrame`] whose
    /// frame/generation IDs are stamped from the review row itself. Only the
    /// Smooth/Retained renderer variant is a GPU scene; any other variant
    /// returns [`RetainedFailureCategory::CaptureUnsupportedVariant`].
    pub(super) fn capture(
        &mut self,
        frame: &PairedReviewFrame,
    ) -> Result<CanonicalRgbaFrame, RetainedFailureCategory> {
        let prepared = frame.prepared();
        let RendererIdentitySource::Smooth {
            metrics,
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            plan,
            draw_order,
        } = prepared.renderer_source()
        else {
            return Err(RetainedFailureCategory::CaptureUnsupportedVariant);
        };

        let background = round_color_to_rgba(prepared.review_background());
        let aperture = prepared.review_aperture();
        let hud = prepared.review_hud();
        let chrome = RetainedChrome {
            mood_aura: round_color_to_rgba(prepared.review_mood_aura()),
            pet_center_col,
            pet_center_row,
            pet_width_cells,
            gauges: prepared.review_gauges(),
            overlays: prepared.review_overlays(),
            hud,
            hud_font_size: prepared.review_hud_font_size(),
            dim_overlay: prepared.review_dim_overlay(),
        };

        // Repaint against the live host's active resource generation — the full
        // preflighted repertoire already holds every glyph the frozen scene
        // paints, so the capture needs no per-frame atlas build.
        let gpu_frame = {
            let active = self
                .host
                .glyph_resources
                .as_ref()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            prepare_gpu_frame(
                plan,
                draw_order,
                metrics,
                aperture,
                background,
                &chrome,
                active.resources.atlas(),
            )?
        };

        let width = self.host.physical_width;
        let height = self.host.physical_height;
        let layout = ReadbackMetadata::for_surface(width, height, self.host.config.format);
        // Reuse the host's persistent capture intermediate/staging and instance
        // ring; a same-size capture allocates nothing.
        self.host.ensure_capture_resources(width, height);
        let mut encoder =
            self.host
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("glorp-retained-capture-encoder"),
                });
        self.host.prepare_frame(&mut encoder, &gpu_frame);
        let capture = self
            .host
            .capture_resources
            .as_ref()
            .expect("ensure_capture_resources installs the persistent capture target");
        {
            let active = self
                .host
                .glyph_resources
                .as_ref()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            self.host.encode_scene(
                &mut encoder,
                &capture.intermediate_view,
                &active.bind_group,
                self.host.frame_buffers.current_buffer(),
                &gpu_frame.blends,
                background,
            );
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &capture.intermediate,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &capture.staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(layout.aligned_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        // One submission, one retained index: the bounded poll below waits on
        // exactly this work to finish before mapping.
        self.host.frame_buffers.finish_uploads();
        let submission = self.host.queue.submit([encoder.finish()]);
        self.host.frame_buffers.recall_uploads();

        let (sender, receiver) = mpsc::sync_channel(1);
        capture
            .staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.host
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            })
            .map_err(|_| RetainedFailureCategory::CapturePollTimeout)?;
        receiver
            .recv_timeout(Duration::from_millis(100))
            .map_err(|_| RetainedFailureCategory::CaptureMapFailed)?
            .map_err(|_| RetainedFailureCategory::CaptureMapFailed)?;
        let mapped = capture
            .staging
            .slice(..)
            .get_mapped_range()
            .map_err(|_| RetainedFailureCategory::CaptureMapFailed)?;
        let premultiplied = normalize_readback_rows(
            &mapped,
            width,
            height,
            layout.aligned_bytes_per_row,
            layout.pixel_order,
        )?;
        drop(mapped);
        capture.staging.unmap();
        // Color seam: the linear-format target stored gamma-premultiplied sRGB
        // color; the canonical artifact is straight sRGB. Unpremultiply once here
        // (a no-op for opaque/transparent pixels).
        let rgba = super::parity::canonical_png_rgba(&premultiplied);

        Ok(CanonicalRgbaFrame {
            frame_id: frame.identity.frame_id,
            resource_generation: frame.identity.resource_generation,
            width,
            height,
            rgba,
        })
    }
}

/// Projects a `RoundColor` into the straight sRGB channel array the scene
/// encoder premultiplies.
fn round_color_to_rgba(color: RoundColor) -> [f32; 4] {
    [color.0, color.1, color.2, color.3]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_rows_are_unpadded_swizzled_and_top_left() {
        let source = vec![
            3, 2, 1, 255, 6, 5, 4, 255, 0, 0, 0, 0, 9, 8, 7, 255, 12, 11, 10, 255, 0, 0, 0, 0,
        ];
        let rgba = normalize_readback_rows(&source, 2, 2, 12, PixelOrder::Bgra).unwrap();
        assert_eq!(
            rgba,
            vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
        );
    }

    #[test]
    fn row_normalization_rejects_short_mapped_buffer() {
        assert!(normalize_readback_rows(&[0; 7], 2, 1, 8, PixelOrder::Rgba).is_err());
    }

    #[test]
    fn rgba_rows_pass_through_without_swizzle() {
        // A padded 1x2 RGBA buffer: two real pixels then 4 padding bytes.
        let source = vec![10, 20, 30, 255, 40, 50, 60, 255, 0, 0, 0, 0];
        let rgba = normalize_readback_rows(&source, 2, 1, 12, PixelOrder::Rgba).unwrap();
        assert_eq!(rgba, vec![10, 20, 30, 255, 40, 50, 60, 255]);
    }

    #[test]
    fn unpadded_row_is_copied_when_stride_equals_width_times_four() {
        // bytes_per_row == width*4: no padding to drop, no swizzle for RGBA.
        let source = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let rgba = normalize_readback_rows(&source, 2, 1, 8, PixelOrder::Rgba).unwrap();
        assert_eq!(rgba, source);
    }

    #[test]
    fn short_buffer_is_rejected_with_the_buffer_too_short_category() {
        assert_eq!(
            normalize_readback_rows(&[0; 7], 2, 1, 8, PixelOrder::Rgba),
            Err(RetainedFailureCategory::CaptureBufferTooShort),
        );
    }

    #[test]
    fn aligned_bytes_per_row_rounds_up_to_the_copy_alignment() {
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        // width*4 == 8 rounds up to one alignment block.
        assert_eq!(aligned_bytes_per_row(2), alignment);
        // An exact multiple stays put: alignment/4 pixels == alignment bytes.
        assert_eq!(aligned_bytes_per_row(alignment / 4), alignment);
        // One pixel past the block rolls to two blocks.
        assert_eq!(aligned_bytes_per_row(alignment / 4 + 1), alignment * 2);
        assert_eq!(aligned_bytes_per_row(0), 0);
    }

    #[test]
    fn readback_metadata_describes_the_padded_row_layout() {
        let layout = ReadbackMetadata::for_surface(2, 3, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert_eq!(layout.physical_width, 2);
        assert_eq!(layout.physical_height, 3);
        assert_eq!(layout.unpadded_bytes_per_row, 8);
        assert_eq!(
            layout.aligned_bytes_per_row,
            wgpu::COPY_BYTES_PER_ROW_ALIGNMENT
        );
        assert_eq!(layout.pixel_order, PixelOrder::Bgra);
        assert_eq!(
            staging_buffer_size(2, 3),
            u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * 3,
        );
    }

    #[test]
    fn rgba_surface_formats_are_not_swizzled() {
        let layout = ReadbackMetadata::for_surface(4, 1, wgpu::TextureFormat::Rgba8UnormSrgb);
        assert_eq!(layout.pixel_order, PixelOrder::Rgba);
    }

    #[test]
    fn capture_failure_categories_are_static_and_hyphenated() {
        for (category, expected) in [
            (
                RetainedFailureCategory::CaptureUnsupportedVariant,
                "retained-capture-unsupported-variant",
            ),
            (
                RetainedFailureCategory::CapturePollTimeout,
                "retained-capture-poll-timeout",
            ),
            (
                RetainedFailureCategory::CaptureMapFailed,
                "retained-capture-map-failed",
            ),
            (
                RetainedFailureCategory::CaptureBufferTooShort,
                "retained-capture-buffer-too-short",
            ),
        ] {
            assert_eq!(category.category(), expected);
        }
    }

    #[test]
    fn scene_readback_layout_is_checked_and_exactly_bgra8_srgb() {
        let layout =
            SceneReadbackLayout::checked(2, 3, wgpu::TextureFormat::Bgra8UnormSrgb, None).unwrap();
        assert_eq!(layout.width(), 2);
        assert_eq!(layout.height(), 3);
        assert_eq!(layout.unpadded_bytes_per_row(), 8);
        assert_eq!(layout.aligned_bytes_per_row(), 256);
        assert_eq!(layout.buffer_size(), 768);

        assert_eq!(
            SceneReadbackLayout::checked(2, 3, wgpu::TextureFormat::Rgba8UnormSrgb, None),
            Err(SceneReadbackError::UnsupportedFormat),
        );
        assert_eq!(
            SceneReadbackLayout::checked(2, 3, wgpu::TextureFormat::Bgra8Unorm, None),
            Err(SceneReadbackError::UnsupportedFormat),
        );
    }

    #[test]
    fn scene_readback_layout_rejects_zero_overflow_and_device_limit() {
        assert_eq!(
            SceneReadbackLayout::checked(0, 1, wgpu::TextureFormat::Bgra8UnormSrgb, None),
            Err(SceneReadbackError::EmptyExtent),
        );
        assert_eq!(
            SceneReadbackLayout::checked(1, 0, wgpu::TextureFormat::Bgra8UnormSrgb, None),
            Err(SceneReadbackError::EmptyExtent),
        );
        assert_eq!(
            SceneReadbackLayout::checked(u32::MAX, 1, wgpu::TextureFormat::Bgra8UnormSrgb, None,),
            Err(SceneReadbackError::RowBytesOverflow),
        );
        assert_eq!(
            SceneReadbackLayout::checked(
                u32::MAX / 4,
                1,
                wgpu::TextureFormat::Bgra8UnormSrgb,
                None,
            ),
            Err(SceneReadbackError::AlignedStrideOverflow),
        );

        let required = 256_u64 * 3;
        assert!(SceneReadbackLayout::checked(
            2,
            3,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            Some(required),
        )
        .is_ok());
        assert_eq!(
            SceneReadbackLayout::checked(
                2,
                3,
                wgpu::TextureFormat::Bgra8UnormSrgb,
                Some(required - 1),
            ),
            Err(SceneReadbackError::BufferLimitExceeded { required, maximum: required - 1 }),
        );
    }

    #[test]
    fn scene_readback_drops_padding_preserves_top_left_rows_and_swizzles_bgra() {
        let layout =
            SceneReadbackLayout::checked(2, 2, wgpu::TextureFormat::Bgra8UnormSrgb, None).unwrap();
        let mut source = vec![0xEE; layout.buffer_size() as usize];
        source[..8].copy_from_slice(&[3, 2, 1, 255, 6, 5, 4, 255]);
        source[256..264].copy_from_slice(&[9, 8, 7, 255, 12, 11, 10, 255]);

        assert_eq!(
            normalize_scene_readback(&source, layout).unwrap(),
            vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
        );
    }

    #[test]
    fn scene_readback_normalizes_premultiplied_linear_color_at_edge_alphas() {
        let layout =
            SceneReadbackLayout::checked(4, 1, wgpu::TextureFormat::Bgra8UnormSrgb, None).unwrap();
        let mut source = vec![0; layout.buffer_size() as usize];
        // Stored bytes are BGRA. RGB is sRGB-encoded premultiplied-linear color.
        source[..16].copy_from_slice(&[
            3, 2, 1, 1, // alpha 1/255
            16, 32, 64, 128, // a representative translucent edge
            1, 127, 253, 254, // alpha 254/255
            201, 101, 51, 0, // hidden RGB must not leak through alpha zero
        ]);

        assert_eq!(
            normalize_scene_readback(&source, layout).unwrap(),
            vec![
                79, 110, 132, 1, // alpha 1
                90, 47, 26, 128, // linear-space unpremultiply, not byte division
                253, 127, 1, 254, // alpha 254
                0, 0, 0, 0, // transparent black regardless of hidden RGB
            ],
        );
    }

    #[test]
    fn scene_readback_rejects_a_short_mapped_buffer() {
        let layout =
            SceneReadbackLayout::checked(1, 1, wgpu::TextureFormat::Bgra8UnormSrgb, None).unwrap();
        assert_eq!(
            normalize_scene_readback(&vec![0; layout.buffer_size() as usize - 1], layout),
            Err(SceneReadbackError::SourceBufferTooShort),
        );
    }

    #[test]
    fn scene_readback_rejects_forged_layout_metadata_without_panicking() {
        let mut layout =
            SceneReadbackLayout::checked(2, 2, wgpu::TextureFormat::Bgra8UnormSrgb, None).unwrap();
        layout.aligned_bytes_per_row = 4;
        layout.buffer_size = 8;

        assert_eq!(
            normalize_scene_readback(&[0; 8], layout),
            Err(SceneReadbackError::InvalidLayout),
        );
    }
}
