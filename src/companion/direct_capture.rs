//! Production artifacts for the direct retained scene route.

use std::path::Path;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::commands::companion_mode::RendererRuntimeState;
use crate::companion::retained::{CompanionRuntimeMetricsSnapshot, DirectSceneCapture};
use crate::error::{GlorpError, Result};
use crate::presentation::companion_scene::contract::{
    CanonicalReadbackArtifact, CanonicalReadbackRequest, CaptureColorSpace, CaptureCompositeAlpha,
    CaptureSourceFormat, CaptureSurfaceArtifact, CompanionCaptureRequest, CompanionCaptureSnapshot,
    CompanionSceneMetricsArtifact, PresentedCapturePrivacy, PresentedSceneVersion,
};
use crate::presentation::privacy::{PresentationSurface, PrivacyProjection};

const DIRECT_CAPTURE_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DirectCaptureNonblank {
    Valid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DirectCaptureValidationError {
    Length,
    AllZero,
    ApertureBlank,
    ControlBlank,
}

#[derive(Serialize)]
struct DirectSceneManifest<'a> {
    schema_version: u16,
    requested_renderer: &'static str,
    effective_renderer: &'static str,
    fallback_occurred: bool,
    fallback_transition_count: u64,
    last_fallback_reason: Option<&'static str>,
    route: &'static str,
    logical_points: [f32; 2],
    physical_pixels: [u32; 2],
    backing_scale: f32,
    receipt: PresentedSceneVersion,
    presented_frame_count: u64,
    last_present_age_ms: u64,
    capture_checksum_sha256: &'a str,
    nonblank_validation: DirectCaptureNonblank,
    privacy_disposition: PresentedCapturePrivacy,
    delta_write_breakdown: &'static str,
    failure_category: Option<&'static str>,
    skip_category: Option<&'static str>,
}

#[derive(Serialize)]
struct SceneVersionArtifact {
    schema_version: u16,
    receipt: PresentedSceneVersion,
}

pub(crate) fn write_direct_scene_capture(
    out_dir: &Path,
    runtime: &RendererRuntimeState,
    capture: &DirectSceneCapture,
    metrics: &CompanionRuntimeMetricsSnapshot,
) -> Result<()> {
    let mut published_metrics = metrics.clone();
    published_metrics.capture_succeeded = published_metrics.capture_succeeded.saturating_add(1);
    published_metrics.capture_nonblank_validated = published_metrics
        .capture_nonblank_validated
        .saturating_add(1);
    validate_direct_rgba(
        capture.receipt.physical_pixels[0],
        capture.receipt.physical_pixels[1],
        &capture.rgba,
    )
    .map_err(|error| {
        GlorpError::Message(format!("direct scene capture validation failed: {error:?}"))
    })?;

    let checksum = sha256_hex(&capture.rgba);
    let surface = CaptureSurfaceArtifact::try_new(
        capture.receipt.logical_points,
        capture.receipt.physical_pixels,
        capture.receipt.backing_scale,
        CaptureSourceFormat::Bgra8UnormSrgb,
        CaptureColorSpace::Srgb,
        CaptureCompositeAlpha::PostMultiplied,
    )
    .map_err(|error| GlorpError::Message(format!("direct capture surface invalid: {error:?}")))?;
    let readback_request =
        CanonicalReadbackRequest::for_physical_pixels(capture.receipt.physical_pixels);
    let readback = CanonicalReadbackArtifact::try_new(
        capture.receipt.physical_pixels,
        capture.receipt.physical_pixels[0]
            .checked_mul(4)
            .ok_or_else(|| GlorpError::Message("direct capture stride overflow".into()))?,
        u64::try_from(capture.rgba.len())
            .map_err(|_| GlorpError::Message("direct capture length overflow".into()))?,
        fnv1a64(&capture.rgba),
    )
    .map_err(|error| GlorpError::Message(format!("direct capture readback invalid: {error:?}")))?;
    let artifact_metrics = CompanionSceneMetricsArtifact::new(
        published_metrics.node_high_water,
        published_metrics.primitive_high_water,
        published_metrics.blended_draw_high_water,
        published_metrics.persistent_gpu_objects_created,
        published_metrics.static_upload_bytes,
        None,
        None,
    );
    let request = CompanionCaptureRequest::try_new(
        capture.receipt.scene_version,
        capture.source,
        capture.logical_state_alias,
        PrivacyProjection::for_surface(PresentationSurface::RoundCompanion),
        capture.receipt.privacy,
        surface,
        readback_request,
        artifact_metrics,
    )
    .map_err(|error| GlorpError::Message(format!("direct capture request invalid: {error:?}")))?;
    let snapshot: CompanionCaptureSnapshot = request
        .complete(capture.receipt.scene_version, readback)
        .map_err(|error| {
            GlorpError::Message(format!("direct capture completion invalid: {error:?}"))
        })?;

    std::fs::create_dir_all(out_dir)?;
    write_rgba_png(
        &out_dir.join("scene.png"),
        capture.receipt.physical_pixels[0],
        capture.receipt.physical_pixels[1],
        &capture.rgba,
    )?;
    let manifest = DirectSceneManifest {
        schema_version: DIRECT_CAPTURE_SCHEMA_VERSION,
        requested_renderer: runtime.requested().forwarded_arg().unwrap_or("auto"),
        effective_renderer: runtime.effective().as_str(),
        fallback_occurred: runtime.transition_count() != 0,
        fallback_transition_count: runtime.transition_count(),
        last_fallback_reason: runtime.last_fallback_reason(),
        route: "direct-retained-scene",
        logical_points: capture.receipt.logical_points,
        physical_pixels: capture.receipt.physical_pixels,
        backing_scale: capture.receipt.backing_scale,
        receipt: capture.receipt,
        presented_frame_count: capture.presented_scene_count,
        last_present_age_ms: capture.last_present_age_ms,
        capture_checksum_sha256: &checksum,
        nonblank_validation: DirectCaptureNonblank::Valid,
        privacy_disposition: capture.receipt.privacy,
        delta_write_breakdown: "not-applicable-aggregate-only-see-scene-metrics",
        failure_category: None,
        skip_category: None,
    };
    write_json(out_dir.join("scene-manifest.json"), &manifest)?;
    write_json(out_dir.join("scene-snapshot.json"), &snapshot)?;
    write_json(
        out_dir.join("scene-version.json"),
        &SceneVersionArtifact {
            schema_version: DIRECT_CAPTURE_SCHEMA_VERSION,
            receipt: capture.receipt,
        },
    )?;
    write_json(out_dir.join("scene-metrics.json"), &published_metrics)?;
    write_json(
        out_dir.join("scene-artifacts.json"),
        &capture.scene_artifacts,
    )?;
    Ok(())
}

pub(crate) fn validate_direct_rgba(
    width: u32,
    height: u32,
    rgba: &[u8],
) -> std::result::Result<DirectCaptureNonblank, DirectCaptureValidationError> {
    let expected = usize::try_from(width)
        .ok()
        .and_then(|width| {
            usize::try_from(height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(DirectCaptureValidationError::Length)?;
    if width == 0 || height == 0 || rgba.len() != expected {
        return Err(DirectCaptureValidationError::Length);
    }
    if !rgba.chunks_exact(4).any(|pixel| pixel != [0, 0, 0, 0]) {
        return Err(DirectCaptureValidationError::AllZero);
    }

    let center = [width / 2, height / 2];
    let aperture_radius = (width.min(height) / 8).max(1);
    if !radial_region_has_alpha(rgba, width, height, center, 0, aperture_radius) {
        return Err(DirectCaptureValidationError::ApertureBlank);
    }

    // The round companion's perimeter controls occupy the outer annulus. Validate
    // that geometry as a region rather than sampling four fragile individual pixels.
    let outer_radius = (width.min(height) / 2).max(1);
    let inner_radius = (outer_radius * 3) / 4;
    if !radial_region_has_alpha(rgba, width, height, center, inner_radius, outer_radius) {
        return Err(DirectCaptureValidationError::ControlBlank);
    }
    Ok(DirectCaptureNonblank::Valid)
}

fn radial_region_has_alpha(
    rgba: &[u8],
    width: u32,
    height: u32,
    center: [u32; 2],
    inner_radius: u32,
    outer_radius: u32,
) -> bool {
    let inner_squared = u64::from(inner_radius).pow(2);
    let outer_squared = u64::from(outer_radius).pow(2);
    (0..height).any(|y| {
        (0..width).any(|x| {
            let dx = i64::from(x) - i64::from(center[0]);
            let dy = i64::from(y) - i64::from(center[1]);
            let distance_squared = dx.unsigned_abs().pow(2) + dy.unsigned_abs().pow(2);
            (inner_squared..=outer_squared).contains(&distance_squared)
                && pixel_alpha(rgba, width, x, y) != 0
        })
    })
}

fn pixel_alpha(rgba: &[u8], width: u32, x: u32, y: u32) -> u8 {
    let offset = (u64::from(y) * u64::from(width) + u64::from(x)) * 4 + 3;
    usize::try_from(offset)
        .ok()
        .and_then(|offset| rgba.get(offset))
        .copied()
        .unwrap_or(0)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3)
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn write_json(path: std::path::PathBuf, value: &impl Serialize) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn write_rgba_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
    let file = std::fs::File::create(path)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(rgba))
        .map_err(|error| GlorpError::Message(format!("direct capture png failed: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_rgba_validation_requires_exact_nonblank_aperture_and_control_regions() {
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        assert_eq!(
            validate_direct_rgba(8, 8, &rgba),
            Err(DirectCaptureValidationError::AllZero)
        );
        rgba[(4 * 8 + 4) * 4 + 3] = 255;
        assert_eq!(
            validate_direct_rgba(8, 8, &rgba),
            Err(DirectCaptureValidationError::ControlBlank)
        );
        rgba[(4 * 8) * 4 + 3] = 255;
        assert_eq!(
            validate_direct_rgba(8, 8, &rgba),
            Ok(DirectCaptureNonblank::Valid)
        );
        assert_eq!(
            validate_direct_rgba(8, 8, &rgba[..rgba.len() - 1]),
            Err(DirectCaptureValidationError::Length)
        );
    }

    #[test]
    fn direct_writer_publishes_required_files_with_one_receipt_and_success_metric() {
        use crate::commands::companion_mode::{
            CompanionRendererRequest, EffectiveCompanionRenderer, RendererRuntimeState,
        };
        use crate::companion::retained::{
            CompanionCapacityInventory, CompanionRuntimeMetrics, RuntimeFixtureIdentity,
            RuntimeIdentity,
        };
        use crate::presentation::companion_scene::{
            AppliedRevisions, DeviceEpoch, LayoutGeneration, ResourceGeneration,
            SceneGenerationKey, SceneVersion, SurfaceEpoch,
        };

        let extent = [8, 8];
        let receipt = PresentedSceneVersion::try_new(
            SceneVersion {
                generation: SceneGenerationKey {
                    device: DeviceEpoch(7),
                    layout: LayoutGeneration(8),
                    resources: ResourceGeneration(9),
                },
                surface: SurfaceEpoch(11),
                applied: AppliedRevisions::new(10, 20),
            },
            [8.0, 8.0],
            extent,
            1.0,
            3,
            PresentedCapturePrivacy::ExternalRedacted,
        )
        .unwrap();
        let mut rgba = vec![0_u8; 8 * 8 * 4];
        rgba[(4 * 8 + 4) * 4..(4 * 8 + 4) * 4 + 4].copy_from_slice(&[1, 2, 3, 255]);
        rgba[(4 * 8) * 4..(4 * 8) * 4 + 4].copy_from_slice(&[4, 5, 6, 255]);
        let capture = crate::companion::retained::direct_scene_capture_fixture(receipt, rgba);
        let runtime = RendererRuntimeState::new(
            CompanionRendererRequest::Retained,
            EffectiveCompanionRenderer::Retained,
        );
        let metrics = CompanionRuntimeMetrics::default().snapshot(
            RuntimeIdentity::baseline(),
            CompanionCapacityInventory::contract_fixture(),
            RuntimeFixtureIdentity {
                fixture_id: "direct-writer-test",
                seed: "fixed",
                update_source: "fixed",
                semantic_cadence_ms: 250,
                logical_width: 8.0,
                logical_height: 8.0,
                physical_width: 8,
                physical_height: 8,
                backing_scale: 1.0,
            },
        );
        let out = tempfile::tempdir().unwrap();

        write_direct_scene_capture(out.path(), &runtime, &capture, &metrics).unwrap();

        for required in [
            "scene.png",
            "scene-manifest.json",
            "scene-snapshot.json",
            "scene-version.json",
            "scene-metrics.json",
            "scene-artifacts.json",
        ] {
            assert!(out.path().join(required).is_file(), "missing {required}");
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.path().join("scene-manifest.json")).unwrap())
                .unwrap();
        let version: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.path().join("scene-version.json")).unwrap())
                .unwrap();
        let snapshot: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.path().join("scene-snapshot.json")).unwrap())
                .unwrap();
        let published_metrics: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.path().join("scene-metrics.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["route"], "direct-retained-scene");
        assert_eq!(manifest["receipt"], version["receipt"]);
        assert_eq!(
            manifest["receipt"]["scene_version"],
            snapshot["request"]["requested_version"]
        );
        assert_eq!(
            manifest["receipt"]["scene_version"],
            snapshot["readback_version"]
        );
        assert_eq!(manifest["privacy_disposition"], "external-redacted");
        assert_eq!(published_metrics["capture_succeeded"], 1);
        assert_eq!(published_metrics["capture_failed"], 0);
        assert_eq!(published_metrics["capture_nonblank_validated"], 1);
        assert!(snapshot["request"]["metrics"]["content_write_bytes"].is_null());
        assert!(snapshot["request"]["metrics"]["frame_write_bytes"].is_null());
    }
}
