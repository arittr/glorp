use super::scene::{
    is_world_blended, NodeId, OrthographicCamera, SceneContent, SceneFrame, SceneTemplate,
};
use super::validate::{validate_full_generation, SceneValidationError};
use super::{GaugeLevelSnapshot, SceneVersion};
use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};

pub const SCENE_ARTIFACT_SCHEMA_VERSION: u16 = 1;
pub const COMPANION_CAPTURE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CaptureSourceIdentity {
    pub template_checksum: u64,
    pub content_checksum: u64,
    pub frame_checksum: u64,
}

impl CaptureSourceIdentity {
    pub const fn new(template_checksum: u64, content_checksum: u64, frame_checksum: u64) -> Self {
        Self {
            template_checksum,
            content_checksum,
            frame_checksum,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompanionCaptureStateAlias {
    Normal,
    Active,
    Asleep,
    HelperTrouble,
    Dim,
}

impl CompanionCaptureStateAlias {
    pub const fn resolve(helper_trouble: bool, asleep: bool, dim: bool, active: bool) -> Self {
        if helper_trouble {
            Self::HelperTrouble
        } else if asleep {
            Self::Asleep
        } else if dim {
            Self::Dim
        } else if active {
            Self::Active
        } else {
            Self::Normal
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureSourceFormat {
    Bgra8Unorm,
    Bgra8UnormSrgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureColorSpace {
    LinearSrgb,
    Srgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureCompositeAlpha {
    Opaque,
    PostMultiplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalReadbackFormat {
    Rgba8UnormSrgb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalReadbackAlpha {
    Straight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CanonicalReadbackOrigin {
    TopLeft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureContractError {
    SchemaVersion,
    NonFinite,
    InvalidExtent,
    InvalidScale,
    ExtentScaleMismatch,
    SourceColorSpaceMismatch,
    SourceCompositeAlphaMismatch,
    ReadbackExtentMismatch,
    ReadbackMetadataMismatch,
    ReadbackStrideMismatch,
    ReadbackLengthMismatch,
    PrivacyViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize)]
pub struct CaptureSurfaceArtifact {
    pub logical_points: [f32; 2],
    pub physical_pixels: [u32; 2],
    pub backing_scale: f32,
    pub source_format: CaptureSourceFormat,
    pub color_space: CaptureColorSpace,
    pub composite_alpha: CaptureCompositeAlpha,
}

impl CaptureSurfaceArtifact {
    pub fn try_new(
        logical_points: [f32; 2],
        physical_pixels: [u32; 2],
        backing_scale: f32,
        source_format: CaptureSourceFormat,
        color_space: CaptureColorSpace,
        composite_alpha: CaptureCompositeAlpha,
    ) -> Result<Self, CaptureContractError> {
        let value = Self {
            logical_points,
            physical_pixels,
            backing_scale,
            source_format,
            color_space,
            composite_alpha,
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(self) -> Result<(), CaptureContractError> {
        if !self.logical_points.into_iter().all(f32::is_finite) || !self.backing_scale.is_finite() {
            return Err(CaptureContractError::NonFinite);
        }
        if self.logical_points.into_iter().any(|value| value <= 0.0)
            || self.physical_pixels.into_iter().any(|value| value == 0)
        {
            return Err(CaptureContractError::InvalidExtent);
        }
        if self.backing_scale <= 0.0 {
            return Err(CaptureContractError::InvalidScale);
        }
        if !matches!(
            (self.source_format, self.color_space),
            (
                CaptureSourceFormat::Bgra8Unorm,
                CaptureColorSpace::LinearSrgb
            ) | (CaptureSourceFormat::Bgra8UnormSrgb, CaptureColorSpace::Srgb)
        ) {
            return Err(CaptureContractError::SourceColorSpaceMismatch);
        }
        if matches!(self.source_format, CaptureSourceFormat::Bgra8UnormSrgb)
            && !matches!(self.composite_alpha, CaptureCompositeAlpha::PostMultiplied)
        {
            return Err(CaptureContractError::SourceCompositeAlphaMismatch);
        }
        for (logical, physical) in self.logical_points.into_iter().zip(self.physical_pixels) {
            let scaled = f64::from(logical) * f64::from(self.backing_scale);
            if !scaled.is_finite() {
                return Err(CaptureContractError::NonFinite);
            }
            if scaled.round() != f64::from(physical) {
                return Err(CaptureContractError::ExtentScaleMismatch);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CanonicalReadbackRequest {
    physical_pixels: [u32; 2],
    format: CanonicalReadbackFormat,
    color_space: CaptureColorSpace,
    alpha: CanonicalReadbackAlpha,
    origin: CanonicalReadbackOrigin,
}

impl CanonicalReadbackRequest {
    pub const fn for_physical_pixels(physical_pixels: [u32; 2]) -> Self {
        Self {
            physical_pixels,
            format: CanonicalReadbackFormat::Rgba8UnormSrgb,
            color_space: CaptureColorSpace::Srgb,
            alpha: CanonicalReadbackAlpha::Straight,
            origin: CanonicalReadbackOrigin::TopLeft,
        }
    }

    pub const fn physical_pixels(self) -> [u32; 2] {
        self.physical_pixels
    }

    pub const fn format(self) -> CanonicalReadbackFormat {
        self.format
    }

    pub const fn color_space(self) -> CaptureColorSpace {
        self.color_space
    }

    pub const fn alpha(self) -> CanonicalReadbackAlpha {
        self.alpha
    }

    pub const fn origin(self) -> CanonicalReadbackOrigin {
        self.origin
    }

    fn validate(self) -> Result<(), CaptureContractError> {
        if self.physical_pixels.into_iter().any(|value| value == 0) {
            return Err(CaptureContractError::InvalidExtent);
        }
        if self.format != CanonicalReadbackFormat::Rgba8UnormSrgb
            || self.color_space != CaptureColorSpace::Srgb
            || self.alpha != CanonicalReadbackAlpha::Straight
            || self.origin != CanonicalReadbackOrigin::TopLeft
        {
            return Err(CaptureContractError::ReadbackMetadataMismatch);
        }
        let bytes_per_row = self.physical_pixels[0]
            .checked_mul(4)
            .ok_or(CaptureContractError::ReadbackStrideMismatch)?;
        u64::from(bytes_per_row)
            .checked_mul(u64::from(self.physical_pixels[1]))
            .ok_or(CaptureContractError::ReadbackLengthMismatch)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CanonicalReadbackArtifact {
    physical_pixels: [u32; 2],
    format: CanonicalReadbackFormat,
    color_space: CaptureColorSpace,
    alpha: CanonicalReadbackAlpha,
    origin: CanonicalReadbackOrigin,
    bytes_per_row: u32,
    byte_len: u64,
    pixel_checksum: u64,
}

impl CanonicalReadbackArtifact {
    pub fn try_new(
        physical_pixels: [u32; 2],
        bytes_per_row: u32,
        byte_len: u64,
        pixel_checksum: u64,
    ) -> Result<Self, CaptureContractError> {
        let request = CanonicalReadbackRequest::for_physical_pixels(physical_pixels);
        request.validate()?;
        let expected_stride = physical_pixels[0]
            .checked_mul(4)
            .ok_or(CaptureContractError::ReadbackStrideMismatch)?;
        if bytes_per_row != expected_stride {
            return Err(CaptureContractError::ReadbackStrideMismatch);
        }
        let expected_len = u64::from(bytes_per_row)
            .checked_mul(u64::from(physical_pixels[1]))
            .ok_or(CaptureContractError::ReadbackLengthMismatch)?;
        if byte_len != expected_len {
            return Err(CaptureContractError::ReadbackLengthMismatch);
        }
        let artifact = Self {
            physical_pixels,
            format: request.format,
            color_space: request.color_space,
            alpha: request.alpha,
            origin: request.origin,
            bytes_per_row,
            byte_len,
            pixel_checksum,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub const fn physical_pixels(self) -> [u32; 2] {
        self.physical_pixels
    }

    pub const fn format(self) -> CanonicalReadbackFormat {
        self.format
    }

    pub const fn color_space(self) -> CaptureColorSpace {
        self.color_space
    }

    pub const fn alpha(self) -> CanonicalReadbackAlpha {
        self.alpha
    }

    pub const fn origin(self) -> CanonicalReadbackOrigin {
        self.origin
    }

    pub const fn bytes_per_row(self) -> u32 {
        self.bytes_per_row
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub const fn pixel_checksum(self) -> u64 {
        self.pixel_checksum
    }

    fn validate(self) -> Result<(), CaptureContractError> {
        CanonicalReadbackRequest {
            physical_pixels: self.physical_pixels,
            format: self.format,
            color_space: self.color_space,
            alpha: self.alpha,
            origin: self.origin,
        }
        .validate()?;
        let expected_stride = self.physical_pixels[0]
            .checked_mul(4)
            .ok_or(CaptureContractError::ReadbackStrideMismatch)?;
        if self.bytes_per_row != expected_stride {
            return Err(CaptureContractError::ReadbackStrideMismatch);
        }
        let expected_len = u64::from(self.bytes_per_row)
            .checked_mul(u64::from(self.physical_pixels[1]))
            .ok_or(CaptureContractError::ReadbackLengthMismatch)?;
        if self.byte_len != expected_len {
            return Err(CaptureContractError::ReadbackLengthMismatch);
        }
        Ok(())
    }

    fn matches_request(
        self,
        request: CanonicalReadbackRequest,
    ) -> Result<(), CaptureContractError> {
        request.validate()?;
        self.validate()?;
        if self.physical_pixels != request.physical_pixels {
            return Err(CaptureContractError::ReadbackExtentMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct CompanionSceneMetricsArtifact {
    pub schema_version: u16,
    pub node_high_water: u32,
    pub primitive_high_water: u32,
    pub blended_draw_high_water: u32,
    pub persistent_gpu_objects_created: u64,
    pub static_upload_bytes: u64,
    pub content_write_bytes: u64,
    pub frame_write_bytes: u64,
}

impl CompanionSceneMetricsArtifact {
    pub const fn new(
        node_high_water: u32,
        primitive_high_water: u32,
        blended_draw_high_water: u32,
        persistent_gpu_objects_created: u64,
        static_upload_bytes: u64,
        content_write_bytes: u64,
        frame_write_bytes: u64,
    ) -> Self {
        Self {
            schema_version: COMPANION_CAPTURE_SCHEMA_VERSION,
            node_high_water,
            primitive_high_water,
            blended_draw_high_water,
            persistent_gpu_objects_created,
            static_upload_bytes,
            content_write_bytes,
            frame_write_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CompanionCaptureRequest {
    pub schema_version: u16,
    pub requested_version: SceneVersion,
    pub source: CaptureSourceIdentity,
    pub logical_state_alias: CompanionCaptureStateAlias,
    pub privacy: PrivacyProjection,
    pub surface: CaptureSurfaceArtifact,
    pub readback: CanonicalReadbackRequest,
    pub metrics: CompanionSceneMetricsArtifact,
}

impl CompanionCaptureRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        requested_version: SceneVersion,
        source: CaptureSourceIdentity,
        logical_state_alias: CompanionCaptureStateAlias,
        privacy: PrivacyProjection,
        surface: CaptureSurfaceArtifact,
        readback: CanonicalReadbackRequest,
        metrics: CompanionSceneMetricsArtifact,
    ) -> Result<Self, CaptureContractError> {
        let request = Self {
            schema_version: COMPANION_CAPTURE_SCHEMA_VERSION,
            requested_version,
            source,
            logical_state_alias,
            privacy,
            surface,
            readback,
            metrics,
        };
        request.validate()?;
        Ok(request)
    }

    pub fn complete(
        self,
        readback_version: SceneVersion,
        readback: CanonicalReadbackArtifact,
    ) -> Result<CompanionCaptureSnapshot, CaptureCompletionError> {
        self.validate()
            .map_err(CaptureCompletionError::RequestContract)?;
        if let Some(mismatch) = scene_version_mismatch(self.requested_version, readback_version) {
            return Err(CaptureCompletionError::Version(mismatch));
        }
        readback
            .matches_request(self.readback)
            .map_err(CaptureCompletionError::ReadbackContract)?;
        Ok(CompanionCaptureSnapshot {
            request: self,
            readback_version,
            readback,
        })
    }

    fn validate(&self) -> Result<(), CaptureContractError> {
        if self.schema_version != COMPANION_CAPTURE_SCHEMA_VERSION
            || self.metrics.schema_version != COMPANION_CAPTURE_SCHEMA_VERSION
        {
            return Err(CaptureContractError::SchemaVersion);
        }
        self.surface.validate()?;
        self.readback.validate()?;
        if self.surface.physical_pixels != self.readback.physical_pixels {
            return Err(CaptureContractError::ReadbackExtentMismatch);
        }
        if !matches!(
            self.privacy.surface,
            PresentationSurface::RoundCompanion
                | PresentationSurface::RoundPreviewLab
                | PresentationSurface::PreviewLabArtifact
        ) || self.privacy != PrivacyProjection::for_surface(self.privacy.surface)
        {
            return Err(CaptureContractError::PrivacyViolation);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneVersionMismatch {
    Device,
    Layout,
    Resources,
    Surface,
    Semantic,
    Frame,
}

fn scene_version_mismatch(
    requested: SceneVersion,
    readback: SceneVersion,
) -> Option<SceneVersionMismatch> {
    if requested.generation.device != readback.generation.device {
        Some(SceneVersionMismatch::Device)
    } else if requested.generation.layout != readback.generation.layout {
        Some(SceneVersionMismatch::Layout)
    } else if requested.generation.resources != readback.generation.resources {
        Some(SceneVersionMismatch::Resources)
    } else if requested.surface != readback.surface {
        Some(SceneVersionMismatch::Surface)
    } else if requested.applied.semantic != readback.applied.semantic {
        Some(SceneVersionMismatch::Semantic)
    } else if requested.applied.frame != readback.applied.frame {
        Some(SceneVersionMismatch::Frame)
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureCompletionError {
    Version(SceneVersionMismatch),
    RequestContract(CaptureContractError),
    ReadbackContract(CaptureContractError),
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CompanionCaptureSnapshot {
    pub request: CompanionCaptureRequest,
    pub readback_version: SceneVersion,
    pub readback: CanonicalReadbackArtifact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SceneArtifactPrivacy {
    ExternalRedacted,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneTemplateArtifact {
    pub schema_version: u16,
    pub scene_schema_version: u16,
    pub renderer_schema_version: u16,
    pub generation_checksum: u64,
    pub privacy: SceneArtifactPrivacy,
    pub primitive_count: usize,
    pub blended_draw_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SceneContentArtifact {
    pub schema_version: u16,
    pub occupied_pet_art_slots: Vec<u16>,
    pub occupied_prop_slots: Vec<u8>,
    pub occupied_tank_slots: Vec<u8>,
    pub active_ambient_slots: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFrameNodeArtifact {
    pub node: NodeId,
    pub visible: bool,
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneFrameArtifact {
    pub schema_version: u16,
    pub camera: OrthographicCamera,
    pub nodes: Vec<SceneFrameNodeArtifact>,
    pub gauges: [GaugeLevelSnapshot; 4],
    pub dimmed: bool,
    pub light_count: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SceneArtifacts {
    pub schema_version: u16,
    pub template: SceneTemplateArtifact,
    pub content: SceneContentArtifact,
    pub frame: SceneFrameArtifact,
}

impl SceneArtifacts {
    pub fn try_from_parts(
        template: &SceneTemplate,
        content: &SceneContent,
        frame: &SceneFrame,
    ) -> Result<Self, SceneValidationError> {
        validate_full_generation(template, content, frame)?;

        let mut occupied_pet_art_slots = content
            .pet_art_slots
            .iter()
            .filter_map(|slot| slot.glyph.map(|_| slot.slot))
            .collect::<Vec<_>>();
        occupied_pet_art_slots.sort_unstable();
        let mut occupied_prop_slots = content
            .prop_slots
            .iter()
            .map(|slot| slot.slot)
            .collect::<Vec<_>>();
        occupied_prop_slots.sort_unstable();
        let mut occupied_tank_slots = content
            .tank_slots
            .iter()
            .map(|slot| slot.slot)
            .collect::<Vec<_>>();
        occupied_tank_slots.sort_unstable();
        let mut active_ambient_slots = content
            .ambient_slots
            .iter()
            .filter(|slot| slot.kind.is_some())
            .map(|slot| slot.slot)
            .collect::<Vec<_>>();
        active_ambient_slots.sort_unstable();
        let mut nodes = frame
            .nodes
            .iter()
            .map(|node| SceneFrameNodeArtifact {
                node: node.node,
                visible: node.visible,
                opacity: node.opacity,
            })
            .collect::<Vec<_>>();
        nodes.sort_by_key(|node| node.node);

        Ok(Self {
            schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
            template: SceneTemplateArtifact {
                schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
                scene_schema_version: template.schema_version,
                renderer_schema_version: template.renderer_schema_version,
                generation_checksum: template.generation_checksum,
                privacy: SceneArtifactPrivacy::ExternalRedacted,
                primitive_count: template.primitives.len(),
                blended_draw_count: template
                    .primitives
                    .iter()
                    .filter(|primitive| {
                        let material = template
                            .materials
                            .iter()
                            .find(|material| material.id == primitive.material)
                            .map(|material| material.kind);
                        is_world_blended(primitive.blend, material)
                    })
                    .count(),
            },
            content: SceneContentArtifact {
                schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
                occupied_pet_art_slots,
                occupied_prop_slots,
                occupied_tank_slots,
                active_ambient_slots,
            },
            frame: SceneFrameArtifact {
                schema_version: SCENE_ARTIFACT_SCHEMA_VERSION,
                camera: frame.camera,
                nodes,
                gauges: frame
                    .gauges
                    .map(|gauge| GaugeLevelSnapshot::from_fraction(f64::from(gauge))),
                dimmed: frame.dim_amount > 0.0,
                light_count: frame.lights.len(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::companion_scene::scene::{
        CanonicalAlias, MaterialKind, SceneFixture, WorldBlend,
    };

    #[test]
    fn artifact_dtos_are_versioned_deterministic_and_privacy_safe() {
        let mut fixture = SceneFixture::valid();
        fixture.template.nodes[0].alias = CanonicalAlias::new("private-node-sentinel").unwrap();
        fixture.template.nodes[0].id = NodeId::from_alias(&fixture.template.nodes[0].alias);
        fixture.template.nodes[1].parent = Some(fixture.template.nodes[0].id);
        fixture.frame.nodes[0].node = fixture.template.nodes[0].id;
        fixture.frame.gauges = [0.123_456_79, 0.234_567_9, 0.345_678_9, 0.456_789];
        fixture.frame.dim_amount = 0.567_891;
        let first =
            SceneArtifacts::try_from_parts(&fixture.template, &fixture.content, &fixture.frame)
                .unwrap();
        let second =
            SceneArtifacts::try_from_parts(&fixture.template, &fixture.content, &fixture.frame)
                .unwrap();
        let first_json = serde_json::to_string(&first).unwrap();
        let second_json = serde_json::to_string(&second).unwrap();
        assert_eq!(first_json, second_json);
        assert!(first_json.contains("\"schema_version\":1"));
        for forbidden in [
            "/Users/",
            "prompt",
            "response",
            "transcript",
            "diagnostic",
            "private-node-sentinel",
            "0.12345679",
            "0.567891",
            "node_aliases",
        ] {
            assert!(
                !first_json.contains(forbidden),
                "artifact leaked {forbidden}"
            );
        }
        assert_eq!(first.frame.gauges[0], super::super::GaugeLevelSnapshot::Low);
        assert!(!format!("{:?}", fixture.template).contains("private-node-sentinel"));
        assert!(!format!("{:?}", fixture.frame).contains("0.12345679"));
        assert!(!format!("{:?}", fixture.frame).contains("0.567891"));
    }

    #[test]
    fn screen_chrome_is_not_counted_as_a_world_blended_draw() {
        let mut fixture = SceneFixture::valid();
        fixture.template.materials[0].kind = MaterialKind::ScreenChrome;
        fixture.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        fixture.template.primitives[0].depth = super::super::scene::DepthBehavior::ScreenNoDepth;
        fixture.template.primitives[0].space = super::super::scene::PrimitiveSpace::Screen;
        let artifacts =
            SceneArtifacts::try_from_parts(&fixture.template, &fixture.content, &fixture.frame)
                .unwrap();
        assert_eq!(artifacts.template.blended_draw_count, 0);
    }

    #[test]
    fn material_aware_blended_limit_and_artifact_count_agree() {
        let mut chrome = SceneFixture::valid();
        chrome.template.materials[0].kind = MaterialKind::ScreenChrome;
        chrome.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        chrome.template.primitives[0].depth = super::super::scene::DepthBehavior::ScreenNoDepth;
        chrome.template.primitives[0].space = super::super::scene::PrimitiveSpace::Screen;
        chrome.template.primitives = vec![chrome.template.primitives[0].clone(); 257];
        for (order, primitive) in chrome.template.primitives.iter_mut().enumerate() {
            primitive.authored_order = order as u16;
        }
        assert!(super::super::validate::validate_template(&chrome.template).is_ok());
        let chrome_artifacts =
            SceneArtifacts::try_from_parts(&chrome.template, &chrome.content, &chrome.frame)
                .unwrap();
        assert_eq!(chrome_artifacts.template.blended_draw_count, 0);

        let mut world = SceneFixture::valid();
        world.template.primitives[0].blend = WorldBlend::PremultipliedAlpha;
        world.template.primitives[0].depth = super::super::scene::DepthBehavior::WorldReadOnly;
        world.template.primitives = vec![world.template.primitives[0].clone(); 257];
        for (order, primitive) in world.template.primitives.iter_mut().enumerate() {
            primitive.authored_order = order as u16;
        }
        assert_eq!(
            super::super::validate::validate_template(&world.template),
            Err(SceneValidationError::BlendedDrawCapacityExceeded)
        );
        world.template.primitives.pop();
        let world_artifacts =
            SceneArtifacts::try_from_parts(&world.template, &world.content, &world.frame).unwrap();
        assert_eq!(world_artifacts.template.blended_draw_count, 256);
    }

    fn capture_version() -> super::super::SceneVersion {
        super::super::SceneVersion {
            generation: super::super::SceneGenerationKey {
                device: super::super::DeviceEpoch(3),
                layout: super::super::LayoutGeneration(5),
                resources: super::super::ResourceGeneration(7),
            },
            surface: super::super::SurfaceEpoch(11),
            applied: super::super::AppliedRevisions::new(13, 17),
        }
    }

    fn capture_surface() -> CaptureSurfaceArtifact {
        CaptureSurfaceArtifact::try_new(
            [360.0, 240.0],
            [720, 480],
            2.0,
            CaptureSourceFormat::Bgra8Unorm,
            CaptureColorSpace::LinearSrgb,
            CaptureCompositeAlpha::Opaque,
        )
        .unwrap()
    }

    fn capture_request() -> CompanionCaptureRequest {
        let surface = capture_surface();
        CompanionCaptureRequest::try_new(
            capture_version(),
            CaptureSourceIdentity::new(19, 23, 29),
            CompanionCaptureStateAlias::Normal,
            crate::presentation::privacy::PrivacyProjection::for_surface(
                crate::presentation::privacy::PresentationSurface::RoundCompanion,
            ),
            surface,
            CanonicalReadbackRequest::for_physical_pixels(surface.physical_pixels),
            CompanionSceneMetricsArtifact::new(31, 37, 41, 43, 47, 53, 59),
        )
        .unwrap()
    }

    #[test]
    fn capture_completion_requires_exact_readback_scene_version() {
        let requested = capture_version();
        let mismatches = [
            (
                SceneVersionMismatch::Device,
                super::super::SceneVersion {
                    generation: super::super::SceneGenerationKey {
                        device: super::super::DeviceEpoch(4),
                        ..requested.generation
                    },
                    ..requested
                },
            ),
            (
                SceneVersionMismatch::Layout,
                super::super::SceneVersion {
                    generation: super::super::SceneGenerationKey {
                        layout: super::super::LayoutGeneration(6),
                        ..requested.generation
                    },
                    ..requested
                },
            ),
            (
                SceneVersionMismatch::Resources,
                super::super::SceneVersion {
                    generation: super::super::SceneGenerationKey {
                        resources: super::super::ResourceGeneration(8),
                        ..requested.generation
                    },
                    ..requested
                },
            ),
            (
                SceneVersionMismatch::Surface,
                super::super::SceneVersion {
                    surface: super::super::SurfaceEpoch(12),
                    ..requested
                },
            ),
            (
                SceneVersionMismatch::Semantic,
                super::super::SceneVersion {
                    applied: super::super::AppliedRevisions::new(14, 17),
                    ..requested
                },
            ),
            (
                SceneVersionMismatch::Frame,
                super::super::SceneVersion {
                    applied: super::super::AppliedRevisions::new(13, 18),
                    ..requested
                },
            ),
        ];
        for (expected, readback_version) in mismatches {
            let request = capture_request();
            let readback =
                CanonicalReadbackArtifact::try_new([720, 480], 2_880, 1_382_400, 61).unwrap();
            assert_eq!(
                request.complete(readback_version, readback),
                Err(CaptureCompletionError::Version(expected))
            );
        }

        let request = capture_request();
        let readback =
            CanonicalReadbackArtifact::try_new([720, 480], 2_880, 1_382_400, 61).unwrap();
        let completed = request.complete(requested, readback).unwrap();
        assert_eq!(completed.request.requested_version, requested);
        assert_eq!(completed.readback_version, requested);
        assert_eq!(completed.readback, readback);
    }

    #[test]
    fn capture_completion_revalidates_the_consumed_request() {
        let mut request = capture_request();
        request.surface.backing_scale = f32::NAN;
        let readback =
            CanonicalReadbackArtifact::try_new([720, 480], 2_880, 1_382_400, 61).unwrap();
        assert_eq!(
            request.complete(capture_version(), readback),
            Err(CaptureCompletionError::RequestContract(
                CaptureContractError::NonFinite
            ))
        );

        let mut request = capture_request();
        request.metrics.schema_version += 1;
        assert_eq!(
            request.complete(capture_version(), readback),
            Err(CaptureCompletionError::RequestContract(
                CaptureContractError::SchemaVersion
            ))
        );
    }

    #[test]
    fn capture_surface_and_readback_geometry_fail_closed() {
        let valid = capture_surface();
        assert_eq!(valid.logical_points, [360.0, 240.0]);
        assert_eq!(valid.physical_pixels, [720, 480]);
        assert_eq!(valid.backing_scale, 2.0);
        for (logical, physical, scale, expected) in [
            (
                [f32::NAN, 240.0],
                [720, 480],
                2.0,
                CaptureContractError::NonFinite,
            ),
            (
                [0.0, 240.0],
                [720, 480],
                2.0,
                CaptureContractError::InvalidExtent,
            ),
            (
                [360.0, 240.0],
                [0, 480],
                2.0,
                CaptureContractError::InvalidExtent,
            ),
            (
                [360.0, 240.0],
                [720, 480],
                f32::INFINITY,
                CaptureContractError::NonFinite,
            ),
            (
                [360.0, 240.0],
                [720, 480],
                0.0,
                CaptureContractError::InvalidScale,
            ),
            (
                [360.0, 240.0],
                [719, 480],
                2.0,
                CaptureContractError::ExtentScaleMismatch,
            ),
            (
                [16_777_216.0, 240.0],
                [16_777_217, 240],
                1.0,
                CaptureContractError::ExtentScaleMismatch,
            ),
        ] {
            assert_eq!(
                CaptureSurfaceArtifact::try_new(
                    logical,
                    physical,
                    scale,
                    CaptureSourceFormat::Bgra8Unorm,
                    CaptureColorSpace::LinearSrgb,
                    CaptureCompositeAlpha::Opaque,
                ),
                Err(expected)
            );
        }

        assert_eq!(
            CanonicalReadbackArtifact::try_new([720, 480], 2_879, 1_382_400, 1),
            Err(CaptureContractError::ReadbackStrideMismatch)
        );
        assert_eq!(
            CanonicalReadbackArtifact::try_new([720, 480], 2_880, 1_382_399, 1),
            Err(CaptureContractError::ReadbackLengthMismatch)
        );
        let mut wrong_extent =
            CanonicalReadbackArtifact::try_new([360, 240], 1_440, 345_600, 1).unwrap();
        wrong_extent.pixel_checksum = 73;
        assert_eq!(
            capture_request().complete(capture_version(), wrong_extent),
            Err(CaptureCompletionError::ReadbackContract(
                CaptureContractError::ReadbackExtentMismatch
            ))
        );
    }

    #[test]
    fn canonical_readback_and_source_formats_are_closed_and_truthful() {
        let request = CanonicalReadbackRequest::for_physical_pixels([720, 480]);
        assert_eq!(request.format, CanonicalReadbackFormat::Rgba8UnormSrgb);
        assert_eq!(request.color_space, CaptureColorSpace::Srgb);
        assert_eq!(request.alpha, CanonicalReadbackAlpha::Straight);
        assert_eq!(request.origin, CanonicalReadbackOrigin::TopLeft);
        assert_ne!(
            CaptureSourceFormat::Bgra8Unorm,
            CaptureSourceFormat::Bgra8UnormSrgb
        );

        assert_eq!(capture_surface().color_space, CaptureColorSpace::LinearSrgb);
        assert_eq!(
            CaptureSurfaceArtifact::try_new(
                [360.0, 240.0],
                [720, 480],
                2.0,
                CaptureSourceFormat::Bgra8Unorm,
                CaptureColorSpace::Srgb,
                CaptureCompositeAlpha::Opaque,
            ),
            Err(CaptureContractError::SourceColorSpaceMismatch)
        );
        assert!(CaptureSurfaceArtifact::try_new(
            [360.0, 240.0],
            [720, 480],
            2.0,
            CaptureSourceFormat::Bgra8UnormSrgb,
            CaptureColorSpace::Srgb,
            CaptureCompositeAlpha::PostMultiplied,
        )
        .is_ok());
        assert_eq!(
            CaptureSurfaceArtifact::try_new(
                [360.0, 240.0],
                [720, 480],
                2.0,
                CaptureSourceFormat::Bgra8UnormSrgb,
                CaptureColorSpace::Srgb,
                CaptureCompositeAlpha::Opaque,
            ),
            Err(CaptureContractError::SourceCompositeAlphaMismatch)
        );

        let formats = serde_json::to_string(&[
            CaptureSourceFormat::Bgra8Unorm,
            CaptureSourceFormat::Bgra8UnormSrgb,
        ])
        .unwrap();
        assert_eq!(formats, r#"["bgra8-unorm","bgra8-unorm-srgb"]"#);
    }

    #[test]
    fn canonical_readback_metadata_cannot_be_weakened() {
        let canonical =
            CanonicalReadbackArtifact::try_new([720, 480], 2_880, 1_382_400, 61).unwrap();

        let mut request = capture_request();
        request.readback.color_space = CaptureColorSpace::LinearSrgb;
        assert_eq!(
            request.complete(capture_version(), canonical),
            Err(CaptureCompletionError::RequestContract(
                CaptureContractError::ReadbackMetadataMismatch
            ))
        );

        let request = capture_request();
        let mut artifact = canonical;
        artifact.color_space = CaptureColorSpace::LinearSrgb;
        assert_eq!(
            request.complete(capture_version(), artifact),
            Err(CaptureCompletionError::ReadbackContract(
                CaptureContractError::ReadbackMetadataMismatch
            ))
        );

        let mut request = capture_request();
        request.readback.color_space = CaptureColorSpace::LinearSrgb;
        let mut artifact = canonical;
        artifact.color_space = CaptureColorSpace::LinearSrgb;
        assert_eq!(
            request.complete(capture_version(), artifact),
            Err(CaptureCompletionError::RequestContract(
                CaptureContractError::ReadbackMetadataMismatch
            ))
        );
    }

    #[test]
    fn canonical_readback_request_preflights_packed_stride_overflow() {
        assert_eq!(
            CanonicalReadbackRequest::for_physical_pixels([u32::MAX, 1]).validate(),
            Err(CaptureContractError::ReadbackStrideMismatch)
        );
    }

    #[test]
    fn logical_capture_state_uses_locked_precedence_without_fault_alias() {
        assert_eq!(
            CompanionCaptureStateAlias::resolve(true, true, true, true),
            CompanionCaptureStateAlias::HelperTrouble
        );
        assert_eq!(
            CompanionCaptureStateAlias::resolve(false, true, true, true),
            CompanionCaptureStateAlias::Asleep
        );
        assert_eq!(
            CompanionCaptureStateAlias::resolve(false, false, true, true),
            CompanionCaptureStateAlias::Dim
        );
        assert_eq!(
            CompanionCaptureStateAlias::resolve(false, false, false, true),
            CompanionCaptureStateAlias::Active
        );
        assert_eq!(
            CompanionCaptureStateAlias::resolve(false, false, false, false),
            CompanionCaptureStateAlias::Normal
        );
        assert!(!serde_json::to_string(&CompanionCaptureStateAlias::Normal)
            .unwrap()
            .contains("fault"));
    }

    #[test]
    fn neutral_capture_identity_and_metrics_serialize_only_closed_safe_fields() {
        let request = capture_request();
        assert_eq!(request.source, CaptureSourceIdentity::new(19, 23, 29));
        let json = serde_json::to_string(&request).unwrap();
        for forbidden in [
            "sentinel-smooth-plan",
            "sentinel-draw-order",
            "sentinel-wall-clock",
            "sentinel-frame-id",
            "sentinel-renderer-generation",
            "sentinel-source-name",
            "sentinel-project-name",
            "/Users/private/sentinel-path",
            "sentinel-diagnostic",
            "sentinel-prompt",
            "sentinel-response",
            "sentinel-exact-token-count",
            "sentinel-user-text",
        ] {
            assert!(!json.contains(forbidden), "capture leaked {forbidden}");
        }
        assert!(!request.privacy.source_names_visible);
        assert!(!request.privacy.exact_counts_visible);
        assert!(!request.privacy.diagnostic_text_visible);
        assert!(!request.privacy.file_paths_visible);
        assert!(!request.privacy.project_names_visible);
        assert!(json.contains("template_checksum"));
        assert!(json.contains("node_high_water"));
        assert!(!json.contains("0.123456"));
    }
}
