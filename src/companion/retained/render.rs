//! Pure contracts and owned CPU preparation for the retained scene renderer.
#![allow(dead_code)] // This checkpoint validates contracts before live GPU materialization.

use super::buffers::ByteSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SceneSurfaceContract {
    pub(super) format: wgpu::TextureFormat,
    pub(super) color_space: wgpu::SurfaceColorSpace,
    pub(super) alpha_mode: wgpu::CompositeAlphaMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneSurfaceContractError {
    MissingBgra8UnormSrgb,
    MissingSrgbColorSpace,
    MissingPostMultipliedAlpha,
    MissingRenderAttachmentUsage,
}

impl SceneSurfaceContract {
    pub(super) fn select(
        capabilities: &wgpu::SurfaceCapabilities,
    ) -> Result<Self, SceneSurfaceContractError> {
        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let format_capabilities = capabilities
            .format_capabilities
            .iter()
            .filter(|candidate| candidate.format == format)
            .collect::<Vec<_>>();
        if format_capabilities.is_empty() {
            return Err(SceneSurfaceContractError::MissingBgra8UnormSrgb);
        }
        if !format_capabilities.iter().any(|candidate| {
            candidate
                .color_spaces
                .contains(wgpu::SurfaceColorSpaces::SRGB)
        }) {
            return Err(SceneSurfaceContractError::MissingSrgbColorSpace);
        }
        if !capabilities
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            return Err(SceneSurfaceContractError::MissingPostMultipliedAlpha);
        }
        if !capabilities
            .usages
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
        {
            return Err(SceneSurfaceContractError::MissingRenderAttachmentUsage);
        }
        Ok(Self {
            format,
            color_space: wgpu::SurfaceColorSpace::Srgb,
            alpha_mode: wgpu::CompositeAlphaMode::PostMultiplied,
        })
    }
}

pub(super) struct SceneTextureContract;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SceneTextureContractError {
    Usage {
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    },
    NotFilterable {
        format: wgpu::TextureFormat,
    },
    NotBlendable {
        format: wgpu::TextureFormat,
    },
}

impl SceneTextureContract {
    pub(super) const INTERMEDIATE: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;
    pub(super) const COVERAGE: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
    pub(super) const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
    pub(super) const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;
    pub(super) const SAMPLE_COUNT: u32 = 1;

    pub(super) fn validate_with(
        mut features_for: impl FnMut(wgpu::TextureFormat) -> wgpu::TextureFormatFeatures,
    ) -> Result<(), SceneTextureContractError> {
        for (format, usages, filterable, blendable) in [
            (
                Self::INTERMEDIATE,
                wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                true,
                true,
            ),
            (
                Self::COVERAGE,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                true,
                false,
            ),
            (
                Self::COLOR,
                wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                true,
                false,
            ),
            (
                Self::DEPTH,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
                false,
                false,
            ),
        ] {
            let features = features_for(format);
            if !features.allowed_usages.contains(usages) {
                return Err(SceneTextureContractError::Usage { format, usage: usages });
            }
            if filterable
                && !features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
            {
                return Err(SceneTextureContractError::NotFilterable { format });
            }
            if blendable
                && !features
                    .flags
                    .contains(wgpu::TextureFormatFeatureFlags::BLENDABLE)
            {
                return Err(SceneTextureContractError::NotBlendable { format });
            }
        }
        Ok(())
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ContentMirrorFamily {
    Globals,
    Pet,
    PropGlyphs,
    TankGlyphs,
    Ambient,
    Hud,
}

impl ContentMirrorFamily {
    pub(super) const ALL: [Self; 6] = [
        Self::Globals,
        Self::Pet,
        Self::PropGlyphs,
        Self::TankGlyphs,
        Self::Ambient,
        Self::Hud,
    ];

    pub(super) const fn record_size(self) -> usize {
        super::compiler::CpuMirrorShape::CONTENT_RECORD_BYTES[self as usize]
    }

    const fn byte_len(self) -> usize {
        self.record_size() * super::compiler::CpuMirrorShape::CONTENT_COUNTS[self as usize]
    }
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FrameMirrorFamily {
    Globals,
    Props,
    TankCells,
    Ambient,
    Hud,
    Lights,
}

impl FrameMirrorFamily {
    pub(super) const ALL: [Self; 6] = [
        Self::Globals,
        Self::Props,
        Self::TankCells,
        Self::Ambient,
        Self::Hud,
        Self::Lights,
    ];

    pub(super) const fn record_size(self) -> usize {
        super::compiler::CpuMirrorShape::FRAME_RECORD_BYTES[self as usize]
    }

    const fn byte_len(self) -> usize {
        self.record_size() * super::compiler::CpuMirrorShape::FRAME_COUNTS[self as usize]
    }
}

pub(super) struct PackedMirrorLayout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PackedMirrorLayoutError {
    MisalignedSpan,
    SpanOutOfBounds,
}

impl PackedMirrorLayout {
    pub(super) const NODE_BYTES: usize = super::compiler::CpuMirrorShape::NODE_RECORD_BYTES
        * super::compiler::CpuMirrorShape::NODE_COUNT;
    pub(super) const CONTENT_BYTES: usize = content_end(ContentMirrorFamily::Hud);
    pub(super) const FRAME_BYTES: usize = frame_end(FrameMirrorFamily::Lights);

    pub(super) const fn content_offset(family: ContentMirrorFamily) -> usize {
        match family {
            ContentMirrorFamily::Globals => 0,
            ContentMirrorFamily::Pet => content_end(ContentMirrorFamily::Globals),
            ContentMirrorFamily::PropGlyphs => content_end(ContentMirrorFamily::Pet),
            ContentMirrorFamily::TankGlyphs => content_end(ContentMirrorFamily::PropGlyphs),
            ContentMirrorFamily::Ambient => content_end(ContentMirrorFamily::TankGlyphs),
            ContentMirrorFamily::Hud => content_end(ContentMirrorFamily::Ambient),
        }
    }

    pub(super) const fn frame_offset(family: FrameMirrorFamily) -> usize {
        match family {
            FrameMirrorFamily::Globals => 0,
            FrameMirrorFamily::Props => frame_end(FrameMirrorFamily::Globals),
            FrameMirrorFamily::TankCells => frame_end(FrameMirrorFamily::Props),
            FrameMirrorFamily::Ambient => frame_end(FrameMirrorFamily::TankCells),
            FrameMirrorFamily::Hud => frame_end(FrameMirrorFamily::Ambient),
            FrameMirrorFamily::Lights => frame_end(FrameMirrorFamily::Hud),
        }
    }

    pub(super) fn translate_node_span(span: ByteSpan) -> Result<ByteSpan, PackedMirrorLayoutError> {
        translate_span(
            0,
            Self::NODE_BYTES,
            super::compiler::CpuMirrorShape::NODE_RECORD_BYTES,
            span,
        )
    }

    pub(super) fn translate_content_span(
        family: ContentMirrorFamily,
        span: ByteSpan,
    ) -> Result<ByteSpan, PackedMirrorLayoutError> {
        translate_span(
            Self::content_offset(family),
            family.byte_len(),
            family.record_size(),
            span,
        )
    }

    pub(super) fn translate_frame_span(
        family: FrameMirrorFamily,
        span: ByteSpan,
    ) -> Result<ByteSpan, PackedMirrorLayoutError> {
        translate_span(
            Self::frame_offset(family),
            family.byte_len(),
            family.record_size(),
            span,
        )
    }
}

const fn content_end(family: ContentMirrorFamily) -> usize {
    PackedMirrorLayout::content_offset(family) + family.byte_len()
}

const fn frame_end(family: FrameMirrorFamily) -> usize {
    PackedMirrorLayout::frame_offset(family) + family.byte_len()
}

fn translate_span(
    family_offset: usize,
    family_len: usize,
    record_size: usize,
    span: ByteSpan,
) -> Result<ByteSpan, PackedMirrorLayoutError> {
    if !span.offset.is_multiple_of(record_size) || !span.len.is_multiple_of(record_size) {
        return Err(PackedMirrorLayoutError::MisalignedSpan);
    }
    let end = span
        .offset
        .checked_add(span.len)
        .ok_or(PackedMirrorLayoutError::SpanOutOfBounds)?;
    if end > family_len {
        return Err(PackedMirrorLayoutError::SpanOutOfBounds);
    }
    Ok(ByteSpan {
        offset: family_offset + span.offset,
        len: span.len,
    })
}

/// IEC 61966-2-1 sRGB electro-optical transfer for scene-owned color math.
pub(super) fn scene_srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// IEC 61966-2-1 sRGB opto-electrical transfer for scene-owned color math.
pub(super) fn scene_linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

pub(super) fn scene_unpremultiply_final(color: [f32; 4]) -> [f32; 4] {
    let alpha = color[3];
    if alpha == 0.0 {
        [0.0; 4]
    } else {
        [color[0] / alpha, color[1] / alpha, color[2] / alpha, alpha]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface_capabilities() -> wgpu::SurfaceCapabilities {
        wgpu::SurfaceCapabilities {
            formats: vec![wgpu::TextureFormat::Bgra8UnormSrgb],
            format_capabilities: vec![wgpu::SurfaceFormatCapabilities {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                color_spaces: wgpu::SurfaceColorSpaces::SRGB,
            }],
            present_modes: vec![wgpu::PresentMode::Fifo],
            alpha_modes: vec![wgpu::CompositeAlphaMode::PostMultiplied],
            usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
        }
    }

    #[test]
    fn scene_surface_contract_selects_only_srgb_postmultiplied_render_attachment() {
        let selected = SceneSurfaceContract::select(&surface_capabilities()).unwrap();
        assert_eq!(selected.format, wgpu::TextureFormat::Bgra8UnormSrgb);
        assert_eq!(selected.color_space, wgpu::SurfaceColorSpace::Srgb);
        assert_eq!(
            selected.alpha_mode,
            wgpu::CompositeAlphaMode::PostMultiplied
        );

        let mut missing_format = surface_capabilities();
        missing_format.format_capabilities.clear();
        assert_eq!(
            SceneSurfaceContract::select(&missing_format),
            Err(SceneSurfaceContractError::MissingBgra8UnormSrgb)
        );

        let mut missing_srgb = surface_capabilities();
        missing_srgb.format_capabilities[0].color_spaces = wgpu::SurfaceColorSpaces::DISPLAY_P3;
        assert_eq!(
            SceneSurfaceContract::select(&missing_srgb),
            Err(SceneSurfaceContractError::MissingSrgbColorSpace)
        );

        let mut missing_alpha = surface_capabilities();
        missing_alpha.alpha_modes = vec![wgpu::CompositeAlphaMode::Opaque];
        assert_eq!(
            SceneSurfaceContract::select(&missing_alpha),
            Err(SceneSurfaceContractError::MissingPostMultipliedAlpha)
        );

        let mut missing_usage = surface_capabilities();
        missing_usage.usages = wgpu::TextureUsages::TEXTURE_BINDING;
        assert_eq!(
            SceneSurfaceContract::select(&missing_usage),
            Err(SceneSurfaceContractError::MissingRenderAttachmentUsage)
        );
    }

    fn required_features(format: wgpu::TextureFormat) -> wgpu::TextureFormatFeatures {
        match format {
            wgpu::TextureFormat::Bgra8UnormSrgb => wgpu::TextureFormatFeatures {
                allowed_usages: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                flags: wgpu::TextureFormatFeatureFlags::FILTERABLE
                    | wgpu::TextureFormatFeatureFlags::BLENDABLE,
            },
            wgpu::TextureFormat::R8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
                wgpu::TextureFormatFeatures {
                    allowed_usages: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST,
                    flags: wgpu::TextureFormatFeatureFlags::FILTERABLE,
                }
            }
            wgpu::TextureFormat::Depth24Plus => wgpu::TextureFormatFeatures {
                allowed_usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
                flags: wgpu::TextureFormatFeatureFlags::empty(),
            },
            _ => unreachable!(),
        }
    }

    #[test]
    fn scene_texture_contract_freezes_formats_and_validates_required_features() {
        assert_eq!(
            SceneTextureContract::INTERMEDIATE,
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
        assert_eq!(SceneTextureContract::COVERAGE, wgpu::TextureFormat::R8Unorm);
        assert_eq!(
            SceneTextureContract::COLOR,
            wgpu::TextureFormat::Rgba8UnormSrgb
        );
        assert_eq!(
            SceneTextureContract::DEPTH,
            wgpu::TextureFormat::Depth24Plus
        );
        assert_eq!(SceneTextureContract::SAMPLE_COUNT, 1);
        SceneTextureContract::validate_with(required_features).unwrap();
        SceneTextureContract::validate_with(|format| {
            format.guaranteed_format_features(wgpu::Features::empty())
        })
        .unwrap();

        let missing_usage = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::COLOR {
                features
                    .allowed_usages
                    .remove(wgpu::TextureUsages::COPY_DST);
            }
            features
        });
        assert!(matches!(
            missing_usage,
            Err(SceneTextureContractError::Usage {
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                ..
            })
        ));

        let missing_intermediate_copy = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::INTERMEDIATE {
                features
                    .allowed_usages
                    .remove(wgpu::TextureUsages::COPY_SRC);
            }
            features
        });
        assert!(matches!(
            missing_intermediate_copy,
            Err(SceneTextureContractError::Usage {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                ..
            })
        ));

        let missing_filterable = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::COVERAGE {
                features
                    .flags
                    .remove(wgpu::TextureFormatFeatureFlags::FILTERABLE);
            }
            features
        });
        assert_eq!(
            missing_filterable,
            Err(SceneTextureContractError::NotFilterable { format: wgpu::TextureFormat::R8Unorm })
        );

        let missing_intermediate_filterable = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::INTERMEDIATE {
                features
                    .flags
                    .remove(wgpu::TextureFormatFeatureFlags::FILTERABLE);
            }
            features
        });
        assert_eq!(
            missing_intermediate_filterable,
            Err(SceneTextureContractError::NotFilterable {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
            })
        );

        let missing_blendable = SceneTextureContract::validate_with(|format| {
            let mut features = required_features(format);
            if format == SceneTextureContract::INTERMEDIATE {
                features
                    .flags
                    .remove(wgpu::TextureFormatFeatureFlags::BLENDABLE);
            }
            features
        });
        assert_eq!(
            missing_blendable,
            Err(SceneTextureContractError::NotBlendable {
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
            })
        );
    }

    #[test]
    fn packed_mirror_layouts_have_frozen_offsets_sizes_and_checked_translation() {
        assert_eq!(PackedMirrorLayout::NODE_BYTES, 12_288);
        assert_eq!(PackedMirrorLayout::CONTENT_BYTES, 10_512);
        assert_eq!(PackedMirrorLayout::FRAME_BYTES, 5_760);
        assert_eq!(
            ContentMirrorFamily::ALL.map(PackedMirrorLayout::content_offset),
            [0, 144, 4_304, 7_184, 7_696, 9_744]
        );
        assert_eq!(
            FrameMirrorFamily::ALL.map(PackedMirrorLayout::frame_offset),
            [0, 192, 672, 1_440, 4_512, 5_664]
        );
        for family in ContentMirrorFamily::ALL {
            let translated = PackedMirrorLayout::translate_content_span(
                family,
                super::super::buffers::ByteSpan { offset: 0, len: family.record_size() },
            )
            .unwrap();
            assert_eq!(
                translated.offset,
                PackedMirrorLayout::content_offset(family)
            );
            assert_eq!(translated.len, family.record_size());
            assert_eq!(translated.offset % 16, 0);
        }
        assert_eq!(
            PackedMirrorLayout::translate_content_span(
                ContentMirrorFamily::Pet,
                super::super::buffers::ByteSpan { offset: 1, len: 32 },
            ),
            Err(PackedMirrorLayoutError::MisalignedSpan)
        );
        assert_eq!(
            PackedMirrorLayout::translate_frame_span(
                FrameMirrorFamily::Lights,
                super::super::buffers::ByteSpan { offset: 96, len: 48 },
            ),
            Err(PackedMirrorLayoutError::SpanOutOfBounds)
        );
        assert_eq!(
            PackedMirrorLayout::translate_node_span(super::super::buffers::ByteSpan {
                offset: 96,
                len: 96,
            }),
            Ok(super::super::buffers::ByteSpan { offset: 96, len: 96 })
        );
    }

    #[test]
    fn scene_color_math_uses_exact_iec_thresholds_and_zero_safe_unpremultiply() {
        assert!((scene_srgb_to_linear(0.04045) - 0.003_130_805).abs() < 1.0e-8);
        assert!((scene_linear_to_srgb(0.003_130_8) - 0.040_449_936).abs() < 1.0e-7);
        assert_eq!(scene_unpremultiply_final([0.0, 0.0, 0.0, 0.0]), [0.0; 4]);
        assert_eq!(
            scene_unpremultiply_final([0.20, 0.10, 0.05, 0.25]),
            [0.80, 0.40, 0.20, 0.25]
        );
    }
}
