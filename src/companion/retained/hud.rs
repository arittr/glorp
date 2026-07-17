//! Renderer-owned preparation, GPU storage, and atomic encoding for the
//! companion's sensitive live HUD.
//!
//! Exact values enter the renderer through [`SealedHudFrame`]. They never enter
//! the semantic scene snapshot, its checksums, or its artifacts. This module
//! turns a sealed live input into fixed-size records that remain sensitive
//! exact-value material: atlas ids, rectangles, and visibility holes fingerprint
//! the live HUD even though their `Debug` output is redacted. A nominal live or
//! redacted encode validates, stages, and records its HUD pass on one caller-owned
//! command encoder, so no safe retained caller can separate upload from draw.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use bytemuck::{Pod, Zeroable};

use super::resources::{GlyphAtlasEntry, GlyphKey, PreparedSceneAtlas};
use crate::presentation::companion_scene::ResourceGeneration;
use crate::round::hud::{
    hud_line_font_sizes, pack_companion_hud_glyphs, prepare_hud_layout, review_capture_hud_text,
    CompanionHudLineRole, CompanionHudText, HudLineMetrics, PackedCompanionHudGlyphs, StatGap,
    COMPANION_HUD_GLYPH_REPERTOIRE, HUD_STACK_INITIAL_SCALE, HUD_STACK_MIN,
    MAX_COMPANION_HUD_GLYPHS,
};

pub(super) const HUD_GPU_BUFFER_BYTES: u64 =
    (MAX_COMPANION_HUD_GLYPHS * std::mem::size_of::<HudGlyphGpuValue>()) as u64;
pub(super) const HUD_GPU_DRAW_INSTANCES: u32 = MAX_COMPANION_HUD_GLYPHS as u32;
pub(super) const HUD_INTERACTION_GPU_BYTES: u64 =
    std::mem::size_of::<HudInteractionGpuValue>() as u64;

pub(super) struct HudGpuBufferUsages;

impl HudGpuBufferUsages {
    pub(super) const RECORDS: wgpu::BufferUsages =
        wgpu::BufferUsages::STORAGE.union(wgpu::BufferUsages::COPY_DST);
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum HudGpuStagingError {
    ResourceGenerationMismatch,
    InvalidInteractionPlan,
    InvalidPrivateSpatialCue,
}

impl fmt::Debug for HudGpuStagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceGenerationMismatch => {
                formatter.write_str("HudGpuStagingError::ResourceGenerationMismatch")
            }
            Self::InvalidInteractionPlan => {
                formatter.write_str("HudGpuStagingError::InvalidInteractionPlan")
            }
            Self::InvalidPrivateSpatialCue => {
                formatter.write_str("HudGpuStagingError::InvalidPrivateSpatialCue")
            }
        }
    }
}

impl fmt::Display for HudGpuStagingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceGenerationMismatch => {
                formatter.write_str("companion HUD GPU generation mismatch")
            }
            Self::InvalidInteractionPlan => {
                formatter.write_str("companion HUD interaction plan is invalid")
            }
            Self::InvalidPrivateSpatialCue => {
                formatter.write_str("companion private spatial cue is invalid")
            }
        }
    }
}

impl std::error::Error for HudGpuStagingError {}

/// The fixed bindings consumed by the private atomic HUD encoder.
///
/// Fields are intentionally private: this value can select the retained
/// resources for an encode, but it cannot expose or replay a raw HUD bind group.
pub(super) struct HudDrawBindings<'resources> {
    coverage_pipeline: &'resources wgpu::RenderPipeline,
    visible_pipeline: &'resources wgpu::RenderPipeline,
    scene: &'resources wgpu::BindGroup,
    atlas: &'resources wgpu::BindGroup,
}

struct HudRenderTarget<'resources> {
    color: &'resources wgpu::TextureView,
    statistics_coverage: &'resources wgpu::TextureView,
    depth: &'resources wgpu::TextureView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HudDrawPhase {
    Coverage,
    Echo,
    Primary,
}

impl<'resources> HudDrawBindings<'resources> {
    pub(super) fn new(
        coverage_pipeline: &'resources wgpu::RenderPipeline,
        visible_pipeline: &'resources wgpu::RenderPipeline,
        scene: &'resources wgpu::BindGroup,
        atlas: &'resources wgpu::BindGroup,
    ) -> Self {
        Self {
            coverage_pipeline,
            visible_pipeline,
            scene,
            atlas,
        }
    }
}

/// Nominal marker for exact live-value HUD material.
pub(crate) struct LiveHudProjection;

/// Nominal marker for the independently constructed, static capture redaction.
pub(crate) struct RedactedCaptureHudProjection;

/// The only owner of exact HUD glyphs inside the renderer.
///
/// The live constructor accepts the existing formatted HUD type and immediately
/// validates and packs it; no method exposes those packed slots. A capture
/// constructs its redacted input afresh, so it cannot borrow or clone a live
/// frame through this API.
pub(crate) struct SealedHudFrame<Projection> {
    packed: PackedCompanionHudGlyphs,
    projection: PhantomData<fn() -> Projection>,
}

impl SealedHudFrame<LiveHudProjection> {
    pub(crate) fn from_live(text: &CompanionHudText) -> Result<Self, HudPreparationError> {
        Self::seal(text)
    }
}

impl SealedHudFrame<RedactedCaptureHudProjection> {
    pub(crate) fn redacted_capture() -> Result<Self, HudPreparationError> {
        let redacted = review_capture_hud_text();
        Self::seal(&redacted)
    }
}

impl<Projection> SealedHudFrame<Projection> {
    fn seal(text: &CompanionHudText) -> Result<Self, HudPreparationError> {
        let packed =
            pack_companion_hud_glyphs(text).map_err(|_| HudPreparationError::InvalidHudContract)?;
        Ok(Self { packed, projection: PhantomData })
    }
}

impl fmt::Debug for SealedHudFrame<LiveHudProjection> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealedHudFrame<Live>(<private>)")
    }
}

impl fmt::Debug for SealedHudFrame<RedactedCaptureHudProjection> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SealedHudFrame<RedactedCapture>(<redacted>)")
    }
}

/// One fixed companion-HUD instance record.
///
/// `rect_points` is `[x, y, width, height]` in logical, Y-up points. Invisible
/// records are entirely zero. `glyph_entry_index` is the dense id resolved from
/// the complete regular-weight [`GlyphKey`] in the prepared scene atlas.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
pub(crate) struct HudGlyphGpuValue {
    rect_points: [f32; 4],
    glyph_entry_index: u32,
    role: u32,
    visible: u32,
    scene_z: f32,
}

/// Fixed renderer-private state for one statistics interaction. It is staged
/// beside, but never packed into, the sealed 32-byte glyph ABI.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Pod, Zeroable)]
pub(super) struct HudInteractionGpuValue {
    reveal_mix: f32,
    enabled: u32,
    _padding: [u32; 2],
}

impl HudInteractionGpuValue {
    fn from_composition(composition: crate::round::depth::CompanionDepthComposition) -> Self {
        Self {
            reveal_mix: composition.statistics_interaction.reveal_mix,
            enabled: u32::from(
                composition.pet_effective_z > composition.statistics_interaction.start_z
                    && composition.pet_effective_z <= composition.statistics_z,
            ),
            _padding: [0; 2],
        }
    }

    pub(super) const fn enabled(self) -> bool {
        self.enabled != 0
    }

    #[cfg(test)]
    pub(super) const fn reveal_mix(self) -> f32 {
        self.reveal_mix
    }
}

impl fmt::Debug for HudInteractionGpuValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HudInteractionGpuValue(<private>)")
    }
}

impl fmt::Debug for HudGlyphGpuValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HudGlyphGpuValue(<private>)")
    }
}

/// Geometry known by the renderer at HUD preparation time.
#[derive(Clone, Copy)]
pub(crate) struct HudPreparationGeometry {
    pub(crate) gap: StatGap,
    pub(crate) aperture_radius: f64,
    pub(crate) view_width: f64,
    pub(crate) view_height: f64,
    pub(crate) hud_font_size: f64,
    pub(crate) resource_generation: ResourceGeneration,
    pub(crate) depth_composition: crate::round::depth::CompanionDepthComposition,
}

impl fmt::Debug for HudPreparationGeometry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HudPreparationGeometry(<private>)")
    }
}

/// Category-only failures at the sealed renderer boundary.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HudPreparationError {
    InvalidHudContract,
    InvalidGeometry,
    InvalidStaticAtlasMetric,
    MissingRegularRepertoireEntry,
    ResourceGenerationMismatch,
}

impl fmt::Debug for HudPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHudContract => {
                formatter.write_str("HudPreparationError::InvalidHudContract")
            }
            Self::InvalidGeometry => formatter.write_str("HudPreparationError::InvalidGeometry"),
            Self::InvalidStaticAtlasMetric => {
                formatter.write_str("HudPreparationError::InvalidStaticAtlasMetric")
            }
            Self::MissingRegularRepertoireEntry => {
                formatter.write_str("HudPreparationError::MissingRegularRepertoireEntry")
            }
            Self::ResourceGenerationMismatch => {
                formatter.write_str("HudPreparationError::ResourceGenerationMismatch")
            }
        }
    }
}

impl fmt::Display for HudPreparationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHudContract => formatter.write_str("companion HUD input is invalid"),
            Self::InvalidGeometry => formatter.write_str("companion HUD geometry is invalid"),
            Self::InvalidStaticAtlasMetric => {
                formatter.write_str("companion HUD atlas metric is invalid")
            }
            Self::MissingRegularRepertoireEntry => {
                formatter.write_str("companion HUD atlas is incomplete")
            }
            Self::ResourceGenerationMismatch => {
                formatter.write_str("companion HUD atlas generation mismatch")
            }
        }
    }
}

impl std::error::Error for HudPreparationError {}

#[derive(Clone, Copy)]
struct HudAtlasMetric {
    entry_index: u32,
    entry: GlyphAtlasEntry,
}

impl fmt::Debug for HudAtlasMetric {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HudAtlasMetric(<static>)")
    }
}

/// Static regular-weight HUD metrics resolved from one scene resource generation.
pub(crate) struct PreparedHudAtlas {
    resource_identity: ResourceGeneration,
    metrics: BTreeMap<char, HudAtlasMetric>,
}

impl PreparedHudAtlas {
    pub(super) fn from_scene_atlas(
        atlas: &PreparedSceneAtlas,
    ) -> Result<Self, HudPreparationError> {
        let mut metrics = BTreeMap::new();
        for &glyph in COMPANION_HUD_GLYPH_REPERTOIRE {
            let key = GlyphKey::new(glyph.to_string(), false);
            let resolved = atlas
                .resolve_key(&key)
                .map_err(|_| HudPreparationError::MissingRegularRepertoireEntry)?;
            validate_static_metric(resolved.entry)?;
            metrics.insert(
                glyph,
                HudAtlasMetric {
                    entry_index: resolved.id,
                    entry: resolved.entry,
                },
            );
        }
        Ok(Self {
            resource_identity: atlas.resource_generation,
            metrics,
        })
    }

    /// Prepares exact live-value records. The output remains sensitive: atlas
    /// ids, rectangles, and visibility holes fingerprint the live HUD even
    /// though the source strings are absent and diagnostics are redacted.
    pub(crate) fn prepare_sensitive(
        &self,
        sealed: &SealedHudFrame<LiveHudProjection>,
        geometry: HudPreparationGeometry,
    ) -> Result<SensitivePreparedHudFrame, HudPreparationError> {
        let prepared = self.prepare_records(sealed, geometry)?;
        Ok(SensitivePreparedHudFrame::from_records(prepared))
    }

    /// Prepares only the independently constructed static capture redaction.
    /// The nominal output cannot be confused with exact live-value material by
    /// future capture APIs.
    pub(crate) fn prepare_redacted_capture(
        &self,
        sealed: &SealedHudFrame<RedactedCaptureHudProjection>,
        geometry: HudPreparationGeometry,
    ) -> Result<CaptureSafePreparedHudFrame, HudPreparationError> {
        let prepared = self.prepare_records(sealed, geometry)?;
        Ok(CaptureSafePreparedHudFrame::from_records(prepared))
    }

    fn prepare_records<Projection>(
        &self,
        sealed: &SealedHudFrame<Projection>,
        geometry: HudPreparationGeometry,
    ) -> Result<PreparedHudRecords, HudPreparationError> {
        validate_geometry(geometry)?;
        if geometry.resource_generation != self.resource_identity {
            return Err(HudPreparationError::ResourceGenerationMismatch);
        }

        let mut lines = HudMetricLines::new();
        for glyph in sealed.packed.occupied_glyphs() {
            let metric = self
                .metrics
                .get(&glyph.glyph)
                .copied()
                .ok_or(HudPreparationError::MissingRegularRepertoireEntry)?;
            lines.push(role_index(glyph.role), metric)?;
        }

        validate_layout_work(&lines, geometry)?;

        let layout = prepare_hud_layout(
            geometry.gap,
            geometry.aperture_radius,
            geometry.view_height,
            geometry.hud_font_size,
            |font_sizes| {
                std::array::from_fn(|line_index| {
                    measure_line(lines.line(line_index), font_sizes[line_index] as f32)
                })
            },
        );
        validate_prepared_layout(layout)?;
        let scene_z = crate::round::depth::CompanionDepthComposition::resolve(0.0)
            .expect("validated shared companion depth composition")
            .statistics_z;

        let mut records = [HudGlyphGpuValue::zeroed(); MAX_COMPANION_HUD_GLYPHS];
        let mut record_index = 0;
        for line_index in 0..HudMetricLines::LINE_COUNT {
            let placed_line = layout.lines[line_index];
            let font_size = placed_line.font_size as f32;
            let mut pen_x = placed_line.origin_x;
            for metric in lines.line(line_index) {
                if let Some(rect_points) = super::glyph_ink_rect(
                    [pen_x as f32, placed_line.baseline_y as f32],
                    font_size,
                    metric.entry,
                ) {
                    if !rect_points.iter().all(|value| value.is_finite()) {
                        return Err(HudPreparationError::InvalidGeometry);
                    }
                    records[record_index] = HudGlyphGpuValue {
                        rect_points,
                        glyph_entry_index: metric.entry_index,
                        role: line_index as u32,
                        visible: 1,
                        scene_z,
                    };
                }
                pen_x += f64::from(super::glyph_advance(metric.entry, font_size));
                record_index += 1;
            }
        }

        Ok(PreparedHudRecords {
            records,
            interaction: HudInteractionGpuValue::from_composition(geometry.depth_composition),
            draw_count: MAX_COMPANION_HUD_GLYPHS as u32,
            resource_identity: self.resource_identity,
        })
    }

    #[cfg(test)]
    pub(super) fn shader_contract_fixture_for_test(&self) -> CaptureSafePreparedHudFrame {
        let mut records = [HudGlyphGpuValue::zeroed(); MAX_COMPANION_HUD_GLYPHS];
        let scene_z = crate::round::depth::CompanionDepthComposition::resolve(0.0)
            .expect("validated shared companion depth composition")
            .statistics_z;
        for (slot, (glyph, rect_points, role)) in [
            ('r', [32.0, 32.0, 16.0, 16.0], 0),
            ('e', [64.0, 32.0, 16.0, 16.0], 0),
            ('e', [96.0, 32.0, 16.0, 16.0], 1),
        ]
        .into_iter()
        .enumerate()
        {
            records[slot] = HudGlyphGpuValue {
                rect_points,
                glyph_entry_index: self
                    .metrics
                    .get(&glyph)
                    .expect("shader contract fixture glyph is preflighted")
                    .entry_index,
                role,
                visible: 1,
                scene_z,
            };
        }
        CaptureSafePreparedHudFrame {
            records,
            interaction: HudInteractionGpuValue::from_composition(
                crate::round::depth::CompanionDepthComposition::resolve(0.0)
                    .expect("validated test depth"),
            ),
            draw_count: HUD_GPU_DRAW_INSTANCES,
            resource_generation: self.resource_identity,
        }
    }
}

impl fmt::Debug for PreparedHudAtlas {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PreparedHudAtlas(<static>)")
    }
}

/// Candidate-owned GPU state for the renderer-only HUD sidecar.
///
/// The two buffers are deliberately separate rather than suballocated: the
/// fixed 832-byte extent is not a portable storage-buffer offset alignment.
/// Both begin as canonical zero data by WebGPU initialization rules and are
/// updated only through the caller-owned staging belt.
pub(super) struct GpuHudResources {
    prepared_atlas: PreparedHudAtlas,
    live_buffer: wgpu::Buffer,
    redacted_buffer: wgpu::Buffer,
    live_interaction_buffer: wgpu::Buffer,
    redacted_interaction_buffer: wgpu::Buffer,
    live_bind_group: wgpu::BindGroup,
    redacted_bind_group: wgpu::BindGroup,
    #[cfg(test)]
    sensitive_copies: u64,
    #[cfg(test)]
    redacted_copies: u64,
    #[cfg(test)]
    copied_bytes: u64,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HudStagingFacts {
    pub(super) sensitive_copies: u64,
    pub(super) redacted_copies: u64,
    pub(super) copied_bytes: u64,
}

impl GpuHudResources {
    pub(super) fn create_unscoped(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        prepared_atlas: PreparedHudAtlas,
    ) -> Self {
        let live_buffer = create_hud_buffer(device, "glorp-scene-live-hud-records");
        let redacted_buffer = create_hud_buffer(device, "glorp-scene-redacted-hud-records");
        let live_interaction_buffer =
            create_hud_interaction_buffer(device, "glorp-scene-live-hud-interaction");
        let redacted_interaction_buffer =
            create_hud_interaction_buffer(device, "glorp-scene-redacted-hud-interaction");
        let live_bind_group = create_hud_bind_group(
            device,
            layout,
            &live_buffer,
            &live_interaction_buffer,
            "glorp-scene-live-hud-bind-group",
        );
        let redacted_bind_group = create_hud_bind_group(
            device,
            layout,
            &redacted_buffer,
            &redacted_interaction_buffer,
            "glorp-scene-redacted-hud-bind-group",
        );
        Self {
            prepared_atlas,
            live_buffer,
            redacted_buffer,
            live_interaction_buffer,
            redacted_interaction_buffer,
            live_bind_group,
            redacted_bind_group,
            #[cfg(test)]
            sensitive_copies: 0,
            #[cfg(test)]
            redacted_copies: 0,
            #[cfg(test)]
            copied_bytes: 0,
        }
    }

    pub(super) fn prepared_atlas(&self) -> &PreparedHudAtlas {
        &self.prepared_atlas
    }

    pub(super) fn bind_sensitive_interaction<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
    ) {
        pass.set_bind_group(3, &self.live_bind_group, &[]);
    }

    pub(super) fn bind_redacted_interaction<'pass>(
        &'pass self,
        pass: &mut wgpu::RenderPass<'pass>,
    ) {
        pass.set_bind_group(3, &self.redacted_bind_group, &[]);
    }

    /// Creates the two private group-3 variants used by the full-screen rim
    /// reveal.  The fragment consumes only the sealed interaction scalar and
    /// the renderer-owned spatial uniform; it never receives HUD records.
    pub(super) fn create_spatial_cue_interaction_bind_groups(
        &self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        spatial_cue_buffer: &wgpu::Buffer,
    ) -> [wgpu::BindGroup; 2] {
        let create = |label, interaction_buffer: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: interaction_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: spatial_cue_buffer.as_entire_binding(),
                    },
                ],
            })
        };
        [
            create(
                "glorp-scene-live-spatial-cue-interaction-bind-group",
                &self.live_interaction_buffer,
            ),
            create(
                "glorp-scene-redacted-spatial-cue-interaction-bind-group",
                &self.redacted_interaction_buffer,
            ),
        ]
    }

    #[cfg(test)]
    pub(super) fn buffer_contract_for_test(&self) -> [(u64, wgpu::BufferUsages); 2] {
        [
            (self.live_buffer.size(), self.live_buffer.usage()),
            (self.redacted_buffer.size(), self.redacted_buffer.usage()),
        ]
    }

    #[cfg(test)]
    pub(super) const fn staging_facts_for_test(&self) -> HudStagingFacts {
        HudStagingFacts {
            sensitive_copies: self.sensitive_copies,
            redacted_copies: self.redacted_copies,
            copied_bytes: self.copied_bytes,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_sensitive(
        &mut self,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        statistics_coverage: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        bindings: HudDrawBindings<'_>,
        prepared: &SensitivePreparedHudFrame,
        phase: HudDrawPhase,
    ) -> Result<(), HudGpuStagingError> {
        validate_staging_generation(
            self.prepared_atlas.resource_identity,
            prepared.resource_generation,
        )?;
        encode_hud(
            staging_belt,
            encoder,
            HudRenderTarget {
                color: target,
                statistics_coverage,
                depth,
            },
            bindings,
            &self.live_buffer,
            &self.live_interaction_buffer,
            &self.live_bind_group,
            &prepared.records,
            prepared.interaction,
            phase,
        );
        #[cfg(test)]
        if phase == HudDrawPhase::Coverage {
            self.sensitive_copies += 1;
            self.copied_bytes += HUD_GPU_BUFFER_BYTES;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn encode_redacted_capture(
        &mut self,
        staging_belt: &mut wgpu::util::StagingBelt,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        statistics_coverage: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        bindings: HudDrawBindings<'_>,
        prepared: &CaptureSafePreparedHudFrame,
        phase: HudDrawPhase,
    ) -> Result<(), HudGpuStagingError> {
        validate_staging_generation(
            self.prepared_atlas.resource_identity,
            prepared.resource_generation,
        )?;
        encode_hud(
            staging_belt,
            encoder,
            HudRenderTarget {
                color: target,
                statistics_coverage,
                depth,
            },
            bindings,
            &self.redacted_buffer,
            &self.redacted_interaction_buffer,
            &self.redacted_bind_group,
            &prepared.records,
            prepared.interaction,
            phase,
        );
        #[cfg(test)]
        if phase == HudDrawPhase::Coverage {
            self.redacted_copies += 1;
            self.copied_bytes += HUD_GPU_BUFFER_BYTES;
        }
        Ok(())
    }

    /// Pure validation used by capture before it creates an encoder or reserves
    /// staging-belt space. The encode path repeats this check so the sealed HUD
    /// upload remains fail-closed even when called independently.
    pub(super) fn validate_redacted_capture(
        &self,
        prepared: &CaptureSafePreparedHudFrame,
    ) -> Result<(), HudGpuStagingError> {
        validate_staging_generation(
            self.prepared_atlas.resource_identity,
            prepared.resource_generation,
        )
    }

    pub(super) fn validate_sensitive(
        &self,
        prepared: &SensitivePreparedHudFrame,
    ) -> Result<(), HudGpuStagingError> {
        validate_staging_generation(
            self.prepared_atlas.resource_identity,
            prepared.resource_generation,
        )
    }
}

impl fmt::Debug for GpuHudResources {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GpuHudResources(<private>)")
    }
}

fn create_hud_buffer(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: HUD_GPU_BUFFER_BYTES,
        usage: HudGpuBufferUsages::RECORDS,
        mapped_at_creation: false,
    })
}

fn create_hud_interaction_buffer(device: &wgpu::Device, label: &'static str) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: HUD_INTERACTION_GPU_BYTES,
        usage: HudGpuBufferUsages::RECORDS,
        mapped_at_creation: false,
    })
}

fn create_hud_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    buffer: &wgpu::Buffer,
    interaction_buffer: &wgpu::Buffer,
    label: &'static str,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: interaction_buffer.as_entire_binding(),
            },
        ],
    })
}

fn validate_staging_generation(
    resources: ResourceGeneration,
    prepared: ResourceGeneration,
) -> Result<(), HudGpuStagingError> {
    if resources != prepared {
        return Err(HudGpuStagingError::ResourceGenerationMismatch);
    }
    Ok(())
}

fn stage_exact_records(
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Buffer,
    records: &[HudGlyphGpuValue; MAX_COMPANION_HUD_GLYPHS],
) {
    let bytes = bytemuck::cast_slice(records);
    debug_assert_eq!(bytes.len() as u64, HUD_GPU_BUFFER_BYTES);
    let size = wgpu::BufferSize::new(HUD_GPU_BUFFER_BYTES).expect("fixed HUD extent is nonzero");
    let mut view = staging_belt.write_buffer(encoder, target, 0, size);
    view.copy_from_slice(bytes);
}

fn stage_interaction(
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::Buffer,
    interaction: HudInteractionGpuValue,
) {
    let size = wgpu::BufferSize::new(HUD_INTERACTION_GPU_BYTES)
        .expect("fixed HUD interaction extent is nonzero");
    let mut view = staging_belt.write_buffer(encoder, target, 0, size);
    view.copy_from_slice(bytemuck::bytes_of(&interaction));
}

#[allow(clippy::too_many_arguments)]
fn encode_hud(
    staging_belt: &mut wgpu::util::StagingBelt,
    encoder: &mut wgpu::CommandEncoder,
    target: HudRenderTarget<'_>,
    bindings: HudDrawBindings<'_>,
    buffer: &wgpu::Buffer,
    interaction_buffer: &wgpu::Buffer,
    bind_group: &wgpu::BindGroup,
    records: &[HudGlyphGpuValue; MAX_COMPANION_HUD_GLYPHS],
    interaction: HudInteractionGpuValue,
    phase: HudDrawPhase,
) {
    if phase == HudDrawPhase::Coverage {
        stage_exact_records(staging_belt, encoder, buffer, records);
        stage_interaction(staging_belt, encoder, interaction_buffer, interaction);
    }
    let (color_attachments, depth_stencil_attachment, pipeline) = match phase {
        HudDrawPhase::Coverage => (
            vec![Some(wgpu::RenderPassColorAttachment {
                view: target.statistics_coverage,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            None,
            bindings.coverage_pipeline,
        ),
        HudDrawPhase::Echo | HudDrawPhase::Primary => (
            vec![Some(wgpu::RenderPassColorAttachment {
                view: target.color,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            Some(wgpu::RenderPassDepthStencilAttachment {
                view: target.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            bindings.visible_pipeline,
        ),
    };
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("glorp-scene-hud-hook"),
        color_attachments: &color_attachments,
        depth_stencil_attachment,
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bindings.scene, &[]);
    pass.set_bind_group(1, bindings.atlas, &[]);
    pass.set_bind_group(3, bind_group, &[]);
    match phase {
        HudDrawPhase::Coverage | HudDrawPhase::Primary => {
            pass.draw(6..12, 0..HUD_GPU_DRAW_INSTANCES);
        }
        HudDrawPhase::Echo => pass.draw(0..6, 0..HUD_GPU_DRAW_INSTANCES),
    }
}

/// Private common storage used only while constructing one of the two nominal
/// prepared projections. It is never exposed as an untyped prepared frame.
struct PreparedHudRecords {
    records: [HudGlyphGpuValue; MAX_COMPANION_HUD_GLYPHS],
    interaction: HudInteractionGpuValue,
    draw_count: u32,
    resource_identity: ResourceGeneration,
}

/// Exact live-value prepared HUD material. The records contain no strings, but
/// their atlas ids, rectangles, and visible-slot pattern fingerprint the exact
/// live values. Keep this out of capture, checksums, artifacts, and diagnostics.
pub(crate) struct SensitivePreparedHudFrame {
    records: [HudGlyphGpuValue; MAX_COMPANION_HUD_GLYPHS],
    interaction: HudInteractionGpuValue,
    draw_count: u32,
    resource_generation: ResourceGeneration,
}

impl SensitivePreparedHudFrame {
    fn from_records(records: PreparedHudRecords) -> Self {
        Self {
            records: records.records,
            interaction: records.interaction,
            draw_count: records.draw_count,
            resource_generation: records.resource_identity,
        }
    }

    fn byte_len(&self) -> usize {
        std::mem::size_of_val(&self.records)
    }

    pub(super) const fn statistics_interaction(&self) -> HudInteractionGpuValue {
        self.interaction
    }

    #[cfg(test)]
    pub(super) fn set_resource_generation_for_test(
        &mut self,
        resource_generation: ResourceGeneration,
    ) {
        self.resource_generation = resource_generation;
    }
}

impl fmt::Debug for SensitivePreparedHudFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SensitivePreparedHudFrame(<private>)")
    }
}

/// Prepared records for the fixed review-capture redaction. Only a nominally
/// redacted sealed input can produce this output.
pub(crate) struct CaptureSafePreparedHudFrame {
    records: [HudGlyphGpuValue; MAX_COMPANION_HUD_GLYPHS],
    interaction: HudInteractionGpuValue,
    draw_count: u32,
    resource_generation: ResourceGeneration,
}

impl CaptureSafePreparedHudFrame {
    fn from_records(records: PreparedHudRecords) -> Self {
        Self {
            records: records.records,
            interaction: records.interaction,
            draw_count: records.draw_count,
            resource_generation: records.resource_identity,
        }
    }

    fn byte_len(&self) -> usize {
        std::mem::size_of_val(&self.records)
    }

    #[cfg(test)]
    pub(super) fn zeroed_for_test(resource_generation: ResourceGeneration) -> Self {
        Self {
            records: [HudGlyphGpuValue::zeroed(); MAX_COMPANION_HUD_GLYPHS],
            interaction: HudInteractionGpuValue::zeroed(),
            draw_count: HUD_GPU_DRAW_INSTANCES,
            resource_generation,
        }
    }

    #[cfg(test)]
    pub(super) fn zeroed_for_test_at(
        resource_generation: ResourceGeneration,
        pet_effective_z: f32,
    ) -> Self {
        let composition = crate::round::depth::CompanionDepthComposition::resolve(pet_effective_z)
            .expect("test HUD depth is valid");
        Self {
            records: [HudGlyphGpuValue::zeroed(); MAX_COMPANION_HUD_GLYPHS],
            interaction: HudInteractionGpuValue::from_composition(composition),
            draw_count: HUD_GPU_DRAW_INSTANCES,
            resource_generation,
        }
    }

    pub(super) const fn statistics_interaction(&self) -> HudInteractionGpuValue {
        self.interaction
    }
}

impl fmt::Debug for CaptureSafePreparedHudFrame {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CaptureSafePreparedHudFrame(<redacted>)")
    }
}

fn role_index(role: CompanionHudLineRole) -> usize {
    match role {
        CompanionHudLineRole::TodayTotal => 0,
        CompanionHudLineRole::DailyPercent => 1,
        CompanionHudLineRole::Pace => 2,
    }
}

/// Fixed allocation-free metric storage for the three HUD lines.
struct HudMetricLines {
    slots: [[Option<HudAtlasMetric>; MAX_COMPANION_HUD_GLYPHS]; 3],
    counts: [usize; 3],
}

impl HudMetricLines {
    const LINE_CAPACITY: usize = MAX_COMPANION_HUD_GLYPHS;
    const LINE_COUNT: usize = 3;

    fn new() -> Self {
        Self {
            slots: [[None; MAX_COMPANION_HUD_GLYPHS]; 3],
            counts: [0; 3],
        }
    }

    fn push(
        &mut self,
        line_index: usize,
        metric: HudAtlasMetric,
    ) -> Result<(), HudPreparationError> {
        let count = self.counts[line_index];
        if count >= Self::LINE_CAPACITY {
            return Err(HudPreparationError::InvalidHudContract);
        }
        self.slots[line_index][count] = Some(metric);
        self.counts[line_index] += 1;
        Ok(())
    }

    fn line(&self, line_index: usize) -> impl ExactSizeIterator<Item = HudAtlasMetric> + '_ {
        self.slots[line_index][..self.counts[line_index]]
            .iter()
            .map(|slot| slot.expect("occupied fixed HUD metric prefix"))
    }

    fn all_lines_occupied(&self) -> bool {
        self.counts.iter().all(|count| *count > 0)
    }
}

fn measure_line(metrics: impl Iterator<Item = HudAtlasMetric>, font_size: f32) -> HudLineMetrics {
    let (width, height) = metrics.fold((0.0_f32, 0.0_f32), |(width, height), metric| {
        (
            width + super::glyph_advance(metric.entry, font_size),
            height.max(metric.entry.line_height * super::glyph_scale(font_size)),
        )
    });
    HudLineMetrics {
        width: f64::from(width),
        height: f64::from(height),
    }
}

fn validate_geometry(geometry: HudPreparationGeometry) -> Result<(), HudPreparationError> {
    let expected_depth_composition = crate::round::depth::CompanionDepthComposition::resolve(
        geometry.depth_composition.pet_effective_z,
    )
    .map_err(|_| HudPreparationError::InvalidGeometry)?;
    let values = [
        geometry.gap.center_x,
        geometry.gap.baseline_y,
        geometry.gap.max_width,
        geometry.aperture_radius,
        geometry.view_width,
        geometry.view_height,
        geometry.hud_font_size,
    ];
    if !values.iter().all(|value| value.is_finite())
        || !geometry.depth_composition.pet_effective_z.is_finite()
        || !geometry
            .depth_composition
            .statistics_interaction
            .reveal_mix
            .is_finite()
        || !(0.0..=1.0).contains(&geometry.depth_composition.statistics_interaction.reveal_mix)
        || geometry.depth_composition != expected_depth_composition
        || geometry.gap.max_width <= 0.0
        || geometry.aperture_radius <= 0.0
        || geometry.view_width <= 0.0
        || geometry.view_height <= 0.0
        || geometry.hud_font_size <= 0.0
        || geometry.aperture_radius > geometry.view_width.min(geometry.view_height) / 2.0
        || geometry.gap.baseline_y < 0.0
        || geometry.gap.baseline_y > geometry.view_height
        || geometry.gap.center_x - geometry.gap.max_width / 2.0 < 0.0
        || geometry.gap.center_x + geometry.gap.max_width / 2.0 > geometry.view_width
    {
        return Err(HudPreparationError::InvalidGeometry);
    }
    Ok(())
}

fn validate_layout_work(
    lines: &HudMetricLines,
    geometry: HudPreparationGeometry,
) -> Result<(), HudPreparationError> {
    const MAX_HUD_SHRINK_STEPS: f64 = 4_096.0;

    if !lines.all_lines_occupied() {
        return Err(HudPreparationError::InvalidHudContract);
    }
    let initial_stack_size = geometry.hud_font_size * HUD_STACK_INITIAL_SCALE;
    if !initial_stack_size.is_finite()
        || (initial_stack_size - HUD_STACK_MIN).max(0.0) > MAX_HUD_SHRINK_STEPS
    {
        return Err(HudPreparationError::InvalidGeometry);
    }
    for (line_index, size) in hud_line_font_sizes(initial_stack_size)
        .into_iter()
        .enumerate()
    {
        let font_size = size as f32;
        if !font_size.is_finite() || font_size <= 0.0 {
            return Err(HudPreparationError::InvalidGeometry);
        }
        let measured = measure_line(lines.line(line_index), font_size);
        if !measured.width.is_finite()
            || !measured.height.is_finite()
            || measured.width <= 0.0
            || measured.height <= 0.0
        {
            return Err(HudPreparationError::InvalidStaticAtlasMetric);
        }
    }
    Ok(())
}

fn validate_prepared_layout(
    layout: crate::round::hud::PreparedHudLayout,
) -> Result<(), HudPreparationError> {
    if !layout.stack_size.is_finite()
        || layout.stack_size <= 0.0
        || layout.lines.iter().any(|line| {
            ![
                line.origin_x,
                line.baseline_y,
                line.width,
                line.height,
                line.font_size,
            ]
            .iter()
            .all(|value| value.is_finite())
                || line.width <= 0.0
                || line.height <= 0.0
                || line.font_size <= 0.0
        })
    {
        return Err(HudPreparationError::InvalidGeometry);
    }
    Ok(())
}

fn validate_static_metric(entry: GlyphAtlasEntry) -> Result<(), HudPreparationError> {
    let finite = entry
        .visible_uv
        .into_iter()
        .flatten()
        .chain(entry.ink_origin)
        .chain(entry.ink_size)
        .chain([
            entry.line_height,
            entry.advance,
            entry.baseline,
            entry.ascent,
            entry.descent,
        ])
        .chain(entry.raster_size)
        .chain([entry.safe_padding])
        .all(|value| value.is_finite());
    let has_positive_ink = entry.ink_size[0] > 0.0 && entry.ink_size[1] > 0.0;
    let has_any_ink = entry.ink_size[0] != 0.0 || entry.ink_size[1] != 0.0;
    if !finite
        || entry.kind != super::resources::GlyphEntryKind::Mask
        || entry.advance <= 0.0
        || entry.line_height <= 0.0
        || entry.safe_padding < 0.0
        || has_any_ink != has_positive_ink
        || entry.visible_uv.is_some() != has_positive_ink
    {
        return Err(HudPreparationError::InvalidStaticAtlasMetric);
    }

    if let Some([u_min, v_min, u_max, v_max]) = entry.visible_uv {
        let valid_uv = u_min >= 0.0
            && v_min >= 0.0
            && u_min < u_max
            && v_min < v_max
            && u_max <= 1.0
            && v_max <= 1.0;
        let left = entry.ink_origin[0] + entry.safe_padding;
        let top = entry.ink_origin[1] + entry.safe_padding;
        let right = left + entry.ink_size[0];
        let bottom = top + entry.ink_size[1];
        let valid_raster = entry.raster_size[0] > 0.0
            && entry.raster_size[1] > 0.0
            && left >= 0.0
            && top >= 0.0
            && right <= entry.raster_size[0]
            && bottom <= entry.raster_size[1];
        if !valid_uv || !valid_raster {
            return Err(HudPreparationError::InvalidStaticAtlasMetric);
        }
    } else if entry.ink_origin != [0.0; 2]
        || entry.ink_size != [0.0; 2]
        || entry.raster_size != [0.0; 2]
        || entry.safe_padding != 0.0
    {
        return Err(HudPreparationError::InvalidStaticAtlasMetric);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::mem::{align_of, offset_of, size_of};

    use super::*;
    use crate::companion::retained::resources::{
        AtlasCell, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphEntryKind, GlyphKey,
        PreparedSceneAtlas,
    };
    use crate::round::hud::{COMPANION_HUD_GLYPH_REPERTOIRE, MAX_COMPANION_HUD_GLYPHS};

    fn metric_for(glyph: char, cell: AtlasCell) -> GlyphAtlasEntry {
        let advance = match glyph {
            ' ' => 7.0,
            '1' | 'i' => 11.0,
            'w' | 'm' => 23.0,
            _ => 17.0,
        };
        if glyph == ' ' {
            return GlyphAtlasEntry::whitespace(advance, 42.0, cell);
        }
        GlyphAtlasEntry {
            visible_uv: Some([0.0, 0.0, 1.0, 1.0]),
            ink_origin: [2.0, 5.0],
            ink_size: [advance - 3.0, 19.0],
            line_height: 42.0,
            advance,
            kind: GlyphEntryKind::Mask,
            allocated_cell: cell,
            baseline: 31.0,
            ascent: 30.0,
            descent: 8.0,
            raster_size: [48.0, 48.0],
            safe_padding: 3.0,
            font_policy_id: 7,
        }
    }

    fn prepared_atlas_without(missing: Option<char>) -> PreparedSceneAtlas {
        let glyphs = COMPANION_HUD_GLYPH_REPERTOIRE
            .iter()
            .copied()
            .filter(|glyph| Some(*glyph) != missing)
            .collect::<Vec<_>>();
        let width = u32::try_from(glyphs.len() * 2).unwrap();
        let mut entries = BTreeMap::new();
        for (index, glyph) in glyphs.into_iter().enumerate() {
            for (weight_index, bold) in [false, true].into_iter().enumerate() {
                let x = u32::try_from(index * 2 + weight_index).unwrap();
                let cell = AtlasCell { origin: [x, 0], extent: [1, 1] };
                entries.insert(
                    GlyphKey::new(glyph.to_string(), bold),
                    metric_for(glyph, cell),
                );
            }
        }
        PreparedSceneAtlas::from_compiled(&CompiledGlyphAtlas {
            width,
            height: 1,
            rgba: vec![0; usize::try_from(width).unwrap() * 4],
            entries,
        })
        .unwrap()
    }

    fn prepared_atlas() -> PreparedSceneAtlas {
        prepared_atlas_without(None)
    }

    fn assert_invalid_static_metric(mutator: fn(&mut GlyphAtlasEntry)) {
        let mut source = prepared_atlas();
        let key = GlyphKey::new("1", false);
        let entry = source
            .entries
            .iter_mut()
            .find(|entry| entry.key == key)
            .expect("regular test glyph");
        mutator(&mut entry.entry);
        let error = PreparedHudAtlas::from_scene_atlas(&source).unwrap_err();
        assert_eq!(error, HudPreparationError::InvalidStaticAtlasMetric);
        assert_eq!(
            format!("{error:?} {error}"),
            "HudPreparationError::InvalidStaticAtlasMetric companion HUD atlas metric is invalid"
        );
    }

    fn geometry() -> HudPreparationGeometry {
        HudPreparationGeometry {
            gap: crate::round::hud::StatGap {
                center_x: 500.0,
                baseline_y: 600.0,
                max_width: 300.0,
            },
            aperture_radius: 480.0,
            view_width: 1_000.0,
            view_height: 1_000.0,
            hud_font_size: 8.0,
            resource_generation: ResourceGeneration(0),
            depth_composition: crate::round::depth::CompanionDepthComposition::resolve(0.0)
                .unwrap(),
        }
    }

    fn prepare_sensitive(
        sealed: &SealedHudFrame<LiveHudProjection>,
        atlas: &PreparedHudAtlas,
    ) -> Result<SensitivePreparedHudFrame, HudPreparationError> {
        atlas.prepare_sensitive(sealed, geometry())
    }

    fn prepare_capture_safe(
        sealed: &SealedHudFrame<RedactedCaptureHudProjection>,
        atlas: &PreparedHudAtlas,
    ) -> Result<CaptureSafePreparedHudFrame, HudPreparationError> {
        atlas.prepare_redacted_capture(sealed, geometry())
    }

    fn live(
        today_tokens: f64,
        daily_fraction: Option<f64>,
        pulse_10m_tokens: f64,
    ) -> SealedHudFrame<LiveHudProjection> {
        let text =
            crate::round::hud::companion_hud_text(today_tokens, daily_fraction, pulse_10m_tokens);
        SealedHudFrame::from_live(&text).expect("production formatter satisfies the HUD contract")
    }

    #[test]
    fn exact_live_values_are_absent_from_diagnostics_and_errors() {
        let sentinel = "918.3M";
        let sealed = live(918_273_645.0, Some(9.17), 64_281.0);
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let prepared = prepare_sensitive(&sealed, &atlas).unwrap();

        let diagnostics = format!("{sealed:?} {atlas:?} {prepared:?}");
        assert!(!diagnostics.contains(sentinel));
        assert_eq!(
            diagnostics,
            "SealedHudFrame<Live>(<private>) PreparedHudAtlas(<static>) SensitivePreparedHudFrame(<private>)"
        );

        let invalid_a = CompanionHudText {
            today_total: format!("{sentinel}🔒"),
            daily_percent: "privacy".into(),
            pace: "redacted".into(),
        };
        let invalid_b = CompanionHudText {
            today_total: "DIFFERENT🔒".into(),
            daily_percent: "privacy".into(),
            pace: "redacted".into(),
        };
        let invalid_a = SealedHudFrame::<LiveHudProjection>::from_live(&invalid_a).unwrap_err();
        let invalid_b = SealedHudFrame::<LiveHudProjection>::from_live(&invalid_b).unwrap_err();
        assert_eq!(
            format!("{invalid_a:?} {invalid_a}"),
            format!("{invalid_b:?} {invalid_b}")
        );
        assert!(!format!("{invalid_a:?} {invalid_a}").contains(sentinel));

        let error =
            PreparedHudAtlas::from_scene_atlas(&prepared_atlas_without(Some('9'))).unwrap_err();
        assert_eq!(
            format!("{error:?}"),
            "HudPreparationError::MissingRegularRepertoireEntry"
        );
        assert_eq!(error.to_string(), "companion HUD atlas is incomplete");
        assert!(!format!("{error:?} {error}").contains('9'));
    }

    #[test]
    fn short_and_maximal_stacks_have_identical_fixed_gpu_extent() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let short = live(0.0, None, 0.0);
        let maximal = live(999_949_999_999.0, Some(99.99), 999_949_999_999.0);
        let short = prepare_sensitive(&short, &atlas).unwrap();
        let maximal = prepare_sensitive(&maximal, &atlas).unwrap();

        assert_eq!(short.records.len(), MAX_COMPANION_HUD_GLYPHS);
        assert_eq!(maximal.records.len(), MAX_COMPANION_HUD_GLYPHS);
        assert_eq!(short.draw_count, MAX_COMPANION_HUD_GLYPHS as u32);
        assert_eq!(maximal.draw_count, MAX_COMPANION_HUD_GLYPHS as u32);
        assert_eq!(maximal.records[MAX_COMPANION_HUD_GLYPHS - 1].visible, 1);
        assert_eq!(short.byte_len(), maximal.byte_len());
        assert_eq!(short.byte_len(), MAX_COMPANION_HUD_GLYPHS * 32);
    }

    #[test]
    fn proportional_metrics_drive_centering_advances_role_sizes_and_y_up_ink_once() {
        let source = prepared_atlas();
        let atlas = PreparedHudAtlas::from_scene_atlas(&source).unwrap();
        let sealed = live(1_100_000.0, Some(0.01), 1_100_000.0);
        let prepared = prepare_sensitive(&sealed, &atlas).unwrap();

        // Production formatting is "1.1M", "1% yday", "1.1M/10m". The first
        // line width is (11 + 17 + 11 + 17) * (8*1.45*1.08 / 48).
        let big_size = 8.0_f32 * 1.45 * 1.08;
        let big_scale = big_size / 48.0;
        let expected_big_width = (11.0 + 17.0 + 11.0 + 17.0) * big_scale;
        let expected_big_origin = 500.0 - expected_big_width / 2.0;
        let first = prepared.records[0];
        assert!((first.rect_points[0] - (expected_big_origin + 2.0 * big_scale)).abs() < 0.0001);
        assert!((first.rect_points[2] - 8.0 * big_scale).abs() < 0.0001);

        // The decimal starts after the narrow "1" advance, proving cumulative
        // proportional advances instead of scalar-index placement.
        let decimal = prepared.records[1];
        assert!(
            (decimal.rect_points[0] - (expected_big_origin + (11.0 + 2.0) * big_scale)).abs()
                < 0.0001
        );

        // First sub-line is centered independently and uses the smaller role size.
        let sub_size = 8.0_f32 * 1.45 * 0.68;
        let sub_scale = sub_size / 48.0;
        let sub_width = (11.0 + 17.0 + 7.0 + 17.0 + 17.0 + 17.0 + 17.0) * sub_scale;
        let sub_origin = 500.0 - sub_width / 2.0;
        let first_sub = prepared.records[4];
        assert!((first_sub.rect_points[0] - (sub_origin + 2.0 * sub_scale)).abs() < 0.0001);
        assert!(first.rect_points[2] > first_sub.rect_points[2]);

        // Space consumes its seven-point advance but is canonical invisible;
        // the following "y" begins after both '%' and the space.
        assert_eq!(prepared.records[6], HudGlyphGpuValue::zeroed());
        let after_space = prepared.records[7];
        assert!(
            (after_space.rect_points[0] - (sub_origin + (11.0 + 17.0 + 7.0 + 2.0) * sub_scale))
                .abs()
                < 0.0001
        );

        // Ink bottom uses the retained top-down raster metrics exactly once to
        // produce a Y-up rect: baseline + (48 - 2*3 - 5 - 19) * scale.
        let total_height = 42.0 * big_scale + 42.0 * sub_scale * 0.82 * 2.0;
        let big_baseline = (1_000.0 - 600.0) + total_height * 0.38;
        let expected_y = big_baseline + (48.0 - 6.0 - 5.0 - 19.0) * big_scale;
        assert!((first.rect_points[1] - expected_y).abs() < 0.0001);
    }

    #[test]
    fn missing_regular_repertoire_entry_fails_with_static_category() {
        let error =
            PreparedHudAtlas::from_scene_atlas(&prepared_atlas_without(Some('w'))).unwrap_err();
        assert_eq!(error, HudPreparationError::MissingRegularRepertoireEntry);
        assert_eq!(
            format!("{error:?}"),
            "HudPreparationError::MissingRegularRepertoireEntry"
        );
        assert!(!format!("{error:?} {error}").contains('w'));
    }

    #[test]
    fn live_changes_do_not_change_independently_built_redacted_capture_records() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let live_a = live(10.0, Some(0.10), 100.0);
        let live_b = live(90_000_000.0, Some(8.0), 7_000_000.0);
        let capture_a = SealedHudFrame::redacted_capture().unwrap();
        let capture_b = SealedHudFrame::redacted_capture().unwrap();

        let live_a = prepare_sensitive(&live_a, &atlas).unwrap();
        let live_b = prepare_sensitive(&live_b, &atlas).unwrap();
        let capture_a = prepare_capture_safe(&capture_a, &atlas).unwrap();
        let capture_b = prepare_capture_safe(&capture_b, &atlas).unwrap();
        let live_a_bytes = bytemuck::cast_slice::<HudGlyphGpuValue, u8>(&live_a.records);
        let live_b_bytes = bytemuck::cast_slice::<HudGlyphGpuValue, u8>(&live_b.records);
        let capture_a_bytes = bytemuck::cast_slice::<HudGlyphGpuValue, u8>(&capture_a.records);
        let capture_b_bytes = bytemuck::cast_slice::<HudGlyphGpuValue, u8>(&capture_b.records);
        assert_ne!(live_a_bytes, live_b_bytes);
        assert_eq!(capture_a_bytes, capture_b_bytes);
        assert_ne!(capture_a_bytes, live_a_bytes);
        assert_ne!(capture_a_bytes, live_b_bytes);
    }

    #[test]
    fn gpu_record_abi_and_invisible_records_are_canonical() {
        assert_eq!(size_of::<HudGlyphGpuValue>(), 32);
        assert_eq!(align_of::<HudGlyphGpuValue>(), 4);
        assert_eq!(offset_of!(HudGlyphGpuValue, rect_points), 0);
        assert_eq!(offset_of!(HudGlyphGpuValue, glyph_entry_index), 16);
        assert_eq!(offset_of!(HudGlyphGpuValue, role), 20);
        assert_eq!(offset_of!(HudGlyphGpuValue, visible), 24);
        assert_eq!(offset_of!(HudGlyphGpuValue, scene_z), 28);

        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let sealed = live(1_100_000.0, Some(0.01), 1_100_000.0);
        let prepared = prepare_sensitive(&sealed, &atlas).unwrap();
        assert!(prepared
            .records
            .iter()
            .filter(|record| record.visible == 1)
            .all(|record| record.scene_z == crate::round::depth::COMPANION_STATISTICS_Z));
        assert_eq!(prepared.records[6], HudGlyphGpuValue::zeroed());
        let last_visible = prepared
            .records
            .iter()
            .rposition(|record| record.visible == 1)
            .unwrap();
        assert!(prepared.records[last_visible + 1..]
            .iter()
            .all(|record| *record == HudGlyphGpuValue::zeroed()));
    }

    #[test]
    fn dedicated_gpu_storage_contract_is_exact() {
        assert_eq!(HUD_GPU_BUFFER_BYTES, 832);
        assert_eq!(HUD_GPU_DRAW_INSTANCES, 26);
        assert_eq!(size_of::<HudInteractionGpuValue>(), 16);
        assert_eq!(
            HudGpuBufferUsages::RECORDS,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST
        );
    }

    #[test]
    fn hud_interaction_state_is_fixed_private_and_redacted() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let sealed = live(12_345.0, Some(0.5), 789.0);
        let mut geometry = geometry();
        geometry.depth_composition =
            crate::round::depth::CompanionDepthComposition::resolve(0.68).unwrap();
        let prepared = atlas.prepare_sensitive(&sealed, geometry).unwrap();

        assert_eq!(size_of::<HudInteractionGpuValue>(), 16);
        assert_eq!(prepared.statistics_interaction().reveal_mix(), 0.5);
        assert!(prepared.statistics_interaction().enabled());
        assert_eq!(
            format!("{prepared:?}"),
            "SensitivePreparedHudFrame(<private>)"
        );
        assert_eq!(
            format!("{:?}", prepared.statistics_interaction()),
            "HudInteractionGpuValue(<private>)"
        );
    }

    #[test]
    fn hud_interaction_rejects_out_of_range_or_incoherent_composition() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let sealed = live(12_345.0, Some(0.5), 789.0);
        let mut out_of_range = geometry();
        out_of_range
            .depth_composition
            .statistics_interaction
            .reveal_mix = 1.01;
        let mut incoherent_boundary = geometry();
        incoherent_boundary
            .depth_composition
            .statistics_interaction
            .start_z = 0.63;

        for invalid in [out_of_range, incoherent_boundary] {
            assert!(matches!(
                atlas.prepare_sensitive(&sealed, invalid),
                Err(HudPreparationError::InvalidGeometry)
            ));
        }
    }

    #[test]
    fn static_resource_identity_and_metrics_ignore_live_values() {
        let source = prepared_atlas();
        let atlas_a = PreparedHudAtlas::from_scene_atlas(&source).unwrap();
        let atlas_b = PreparedHudAtlas::from_scene_atlas(&source).unwrap();
        let before = atlas_a.resource_identity;
        let live_a = live(12.0, Some(0.5), 34.0);
        let live_b = live(98_765_432.0, Some(7.0), 1_234_567.0);
        let first = prepare_sensitive(&live_a, &atlas_a).unwrap();
        let second = prepare_sensitive(&live_b, &atlas_a).unwrap();
        assert_eq!(first.resource_generation, ResourceGeneration(0));
        assert_eq!(second.resource_generation, ResourceGeneration(0));
        assert_eq!(atlas_a.resource_identity, before);
        assert_eq!(atlas_a.resource_identity, atlas_b.resource_identity);
        assert_eq!(format!("{atlas_a:?}"), "PreparedHudAtlas(<static>)");
    }

    #[test]
    fn live_and_redacted_projections_have_distinct_nominal_input_and_output_types() {
        use std::any::TypeId;

        assert_ne!(
            TypeId::of::<SealedHudFrame<LiveHudProjection>>(),
            TypeId::of::<SealedHudFrame<RedactedCaptureHudProjection>>()
        );
        assert_ne!(
            TypeId::of::<SensitivePreparedHudFrame>(),
            TypeId::of::<CaptureSafePreparedHudFrame>()
        );

        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let live = live(12_345.0, Some(0.5), 789.0);
        let redacted = SealedHudFrame::<RedactedCaptureHudProjection>::redacted_capture().unwrap();
        assert_eq!(
            format!("{:?}", prepare_sensitive(&live, &atlas).unwrap()),
            "SensitivePreparedHudFrame(<private>)"
        );
        assert_eq!(
            format!("{:?}", prepare_capture_safe(&redacted, &atlas).unwrap()),
            "CaptureSafePreparedHudFrame(<redacted>)"
        );
    }

    #[test]
    fn preparation_rejects_resource_generation_mismatch_with_static_error() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let live = live(12_345.0, Some(0.5), 789.0);
        let mut mismatched = geometry();
        mismatched.resource_generation = ResourceGeneration(99);
        let error = atlas.prepare_sensitive(&live, mismatched).unwrap_err();
        assert_eq!(error, HudPreparationError::ResourceGenerationMismatch);
        assert_eq!(
            format!("{error:?} {error}"),
            "HudPreparationError::ResourceGenerationMismatch companion HUD atlas generation mismatch"
        );
        assert!(!format!("{error:?} {error}").contains("99"));
    }

    #[test]
    fn nonfinite_and_extreme_geometry_fails_closed_before_layout() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let live = live(12_345.0, Some(0.5), 789.0);
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
            let mut candidate = geometry();
            candidate.hud_font_size = invalid;
            let error = atlas.prepare_sensitive(&live, candidate).unwrap_err();
            assert_eq!(error, HudPreparationError::InvalidGeometry);
            assert_eq!(
                format!("{error:?} {error}"),
                "HudPreparationError::InvalidGeometry companion HUD geometry is invalid"
            );
        }

        let mut candidate = geometry();
        candidate.gap.center_x = f64::NAN;
        assert_eq!(
            atlas.prepare_sensitive(&live, candidate).unwrap_err(),
            HudPreparationError::InvalidGeometry
        );

        for candidate in [
            {
                let mut value = geometry();
                value.view_width = 0.0;
                value
            },
            {
                let mut value = geometry();
                value.view_height = 0.0;
                value
            },
            {
                let mut value = geometry();
                value.aperture_radius = 0.0;
                value
            },
            {
                let mut value = geometry();
                value.hud_font_size = 0.0;
                value
            },
            {
                let mut value = geometry();
                value.gap.max_width = 0.0;
                value
            },
            {
                let mut value = geometry();
                value.gap.baseline_y = value.view_height + 1.0;
                value
            },
            {
                let mut value = geometry();
                value.gap.center_x = 10.0;
                value.gap.max_width = 40.0;
                value
            },
        ] {
            assert_eq!(
                atlas.prepare_sensitive(&live, candidate).unwrap_err(),
                HudPreparationError::InvalidGeometry
            );
        }
    }

    #[test]
    fn malformed_static_metrics_fail_closed_while_whitespace_remains_valid() {
        assert_invalid_static_metric(|entry| entry.advance = 0.0);
        assert_invalid_static_metric(|entry| entry.line_height = 0.0);
        assert_invalid_static_metric(|entry| entry.raster_size[0] = 0.0);
        assert_invalid_static_metric(|entry| entry.visible_uv = Some([0.6, 0.0, 0.4, 1.0]));
        assert_invalid_static_metric(|entry| entry.visible_uv = Some([-0.1, 0.0, 0.4, 1.0]));
        assert_invalid_static_metric(|entry| entry.ink_origin[0] = -4.0);
        assert_invalid_static_metric(|entry| {
            entry.kind = GlyphEntryKind::PremultipliedColorRgba;
        });

        // The declared space is intentionally inkless but still carries positive
        // advance and line height, so its zero raster extent is the sole exception.
        PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
    }

    #[test]
    fn metric_lines_are_fixed_capacity_and_allocation_free() {
        assert!(!std::mem::needs_drop::<HudMetricLines>());
        assert_eq!(HudMetricLines::LINE_CAPACITY, MAX_COMPANION_HUD_GLYPHS);
        assert_eq!(HudMetricLines::LINE_COUNT, 3);
    }

    #[test]
    fn three_hundred_live_frames_keep_fixed_private_extent_and_stable_redaction() {
        let atlas = PreparedHudAtlas::from_scene_atlas(&prepared_atlas()).unwrap();
        let capture = SealedHudFrame::<RedactedCaptureHudProjection>::redacted_capture().unwrap();
        let capture = prepare_capture_safe(&capture, &atlas).unwrap();
        let capture_bytes = bytemuck::cast_slice::<HudGlyphGpuValue, u8>(&capture.records).to_vec();

        for frame_index in 0..300_u64 {
            let live = live(
                ((frame_index * 3_456_789) % 999_949_999_999) as f64,
                Some((frame_index % 1_200) as f64 / 100.0),
                ((frame_index * 98_765) % 999_949_999_999) as f64,
            );
            let prepared = prepare_sensitive(&live, &atlas).unwrap();
            assert_eq!(prepared.records.len(), MAX_COMPANION_HUD_GLYPHS);
            assert_eq!(prepared.byte_len(), MAX_COMPANION_HUD_GLYPHS * 32);
            assert_eq!(prepared.draw_count, MAX_COMPANION_HUD_GLYPHS as u32);
            assert_eq!(prepared.resource_generation, ResourceGeneration(0));
            for record in prepared.records {
                assert!(record.rect_points.iter().all(|value| value.is_finite()));
                if record.visible == 0 {
                    assert_eq!(record, HudGlyphGpuValue::zeroed());
                }
            }

            let redacted =
                SealedHudFrame::<RedactedCaptureHudProjection>::redacted_capture().unwrap();
            let redacted = prepare_capture_safe(&redacted, &atlas).unwrap();
            assert_eq!(
                bytemuck::cast_slice::<HudGlyphGpuValue, u8>(&redacted.records),
                capture_bytes.as_slice()
            );
            assert_eq!(redacted.resource_generation, ResourceGeneration(0));
        }
    }
}
