#![cfg(all(target_os = "macos", feature = "retained-renderer"))]

use std::ffi::c_void;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_foundation::NSSize;
use objc2_quartz_core::CAMetalLayer;
use wgpu::util::DeviceExt;

use crate::presentation::smooth::{
    SmoothBlendMode, SmoothClip, SmoothCompanionLayer, SmoothCompanionScenePlan, SmoothFill,
    SmoothLayerItem, SmoothLayerMotionBinding, SmoothPoint, SmoothRgba8, SmoothShapeGeometry,
};
use crate::round::draw::{RoundColor, RoundDrawCommand, RoundDrawKind};
use crate::round::hud::{
    daily_overage_color, growth_ring_fill_end_deg, perimeter_gauge_colors, perimeter_gauge_layout,
    stat_gap_box, CompanionHudText, GaugeLane, GaugeLaneColors, LineCap, COMPANION_GAUGE_GAP_DEG,
};
use crate::round::layout::RoundAperture;

use super::app::{CompanionGridMetrics, PreparedGaugeFrame};

mod capture;
mod presentation;
mod resources;

pub(crate) use capture::CanonicalRgbaFrame;
pub(crate) use presentation::{
    FrameDisposition, FrameMilestone, FrameProgress, GpuErrorMailbox, RetainedFailureCategory,
    SkipReason,
};
use resources::{
    CompiledGlyphAtlas, CompiledRetainedResources, FragmentGlyphMode, GlyphAtlasEntry, GlyphKey,
    GlyphRepertoireManifest, RETAINED_ATLAS_POINT_SIZE,
};

use crate::round::smooth::CompanionContentIdentity;

/// The point size the retained atlas rasterizes glyphs at; the on-screen quad
/// scale divides the display font size by this. Matches the manifest's
/// production atlas point size.
const GLYPH_FONT_SIZE: f64 = RETAINED_ATLAS_POINT_SIZE;
const TANK_CORE_TINT: [f32; 3] = [0.10, 0.11, 0.20];
const TANK_CORE_TINT_WEIGHT: f32 = 0.42;

pub(super) struct RetainedChrome<'a> {
    pub(super) mood_aura: [f32; 4],
    pub(super) pet_center_col: f64,
    pub(super) pet_center_row: f64,
    pub(super) pet_width_cells: f64,
    pub(super) gauges: PreparedGaugeFrame,
    pub(super) overlays: &'a [RoundDrawCommand],
    pub(super) hud: &'a CompanionHudText,
    pub(super) hud_font_size: f64,
    pub(super) dim_overlay: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuPrimitive {
    rect: [f32; 4],
    color_a: [f32; 4],
    color_b: [f32; 4],
    uv: [f32; 4],
    params: [f32; 4],
    clip_rect: [f32; 4],
    clip_ellipse: [f32; 4],
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
}

impl GpuPrimitive {
    const ATTRIBUTES: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Float32x4,
        8 => Float32x4
    ];

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

struct PreparedGpuFrame {
    primitives: Vec<GpuPrimitive>,
    blends: Vec<SmoothBlendMode>,
}

/// The GPU resources for one active resource generation: the compiled atlas plus
/// the declared-content identity and backing scale it was compiled for, so the
/// host knows when a generation change requires recompiling, and the uploaded
/// texture + bind group.
struct ActiveGlyphResources {
    identity: CompanionContentIdentity,
    backing_scale: f64,
    resources: CompiledRetainedResources,
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

pub(super) struct RetainedHost {
    // Surface must drop before the retained CAMetalLayer.
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    layer: Retained<CAMetalLayer>,
    pipelines: Pipelines,
    atlas_layout: wgpu::BindGroupLayout,
    glyph_resources: Option<ActiveGlyphResources>,
    physical_width: u32,
    physical_height: u32,
    backing_scale: f64,
    frame_counter: u64,
    gpu_errors: GpuErrorMailbox,
}

struct Pipelines {
    normal: wgpu::RenderPipeline,
    multiply: wgpu::RenderPipeline,
    screen: wgpu::RenderPipeline,
    add: wgpu::RenderPipeline,
    replace: wgpu::RenderPipeline,
}

impl Pipelines {
    fn get(&self, blend: SmoothBlendMode) -> &wgpu::RenderPipeline {
        match blend {
            SmoothBlendMode::Normal => &self.normal,
            SmoothBlendMode::Multiply => &self.multiply,
            SmoothBlendMode::Screen => &self.screen,
            SmoothBlendMode::Add => &self.add,
            SmoothBlendMode::Replace => &self.replace,
        }
    }
}

/// Tracks whether the Metal layer is installed on the AppKit view and whether
/// the view still holds its original AppKit layer state. The activation guard
/// consults it to decide whether a dropped, uncommitted activation must roll the
/// attach back.
#[derive(Debug, Default, Clone, Copy)]
struct LayerActivationState {
    attached: bool,
    appkit_restored: bool,
}

impl LayerActivationState {
    /// Records that the Metal layer was installed on the view.
    fn mark_attached(&mut self) {
        self.attached = true;
        self.appkit_restored = false;
    }

    /// Records that preparation failed before the layer was ever installed, so
    /// the view keeps its original AppKit layer untouched.
    #[allow(dead_code)] // Models the never-attached invariant the activation-state tests pin.
    fn preflight_failed(&mut self) {
        self.attached = false;
        self.appkit_restored = true;
    }

    fn attached(&self) -> bool {
        self.attached
    }

    #[allow(dead_code)] // Read by the activation-state tests.
    fn appkit_restored(&self) -> bool {
        self.appkit_restored
    }
}

/// Where a dropped, uncommitted activation guard sends its rollback.
enum ActivationRollback<'a> {
    /// Production rollback restores the view's prior AppKit layer state.
    View(&'a NSView),
    /// Test rollback clears an observable attachment flag.
    #[cfg(test)]
    TestFlag(std::rc::Rc<std::cell::Cell<bool>>),
}

/// RAII guard for the AppKit layer attach performed by
/// [`PreparedRetainedHost::activate`]. Dropping it before
/// [`LayerActivationGuard::commit`] rolls the attach back, so a failure after
/// the layer is installed never leaves the view half-attached.
struct LayerActivationGuard<'a> {
    rollback: ActivationRollback<'a>,
    state: LayerActivationState,
    committed: bool,
}

impl<'a> LayerActivationGuard<'a> {
    /// Arms a guard that restores the view's prior AppKit layer state on drop
    /// until committed.
    fn install(view: &'a NSView) -> Self {
        let mut state = LayerActivationState::default();
        state.mark_attached();
        Self {
            rollback: ActivationRollback::View(view),
            state,
            committed: false,
        }
    }

    /// Test constructor whose rollback clears the supplied attachment flag.
    #[cfg(test)]
    fn for_test(flag: std::rc::Rc<std::cell::Cell<bool>>) -> Self {
        let mut state = LayerActivationState::default();
        state.mark_attached();
        Self {
            rollback: ActivationRollback::TestFlag(flag),
            state,
            committed: false,
        }
    }

    /// Keeps the attach; the guard no longer rolls back on drop.
    fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for LayerActivationGuard<'_> {
    fn drop(&mut self) {
        if self.committed || !self.state.attached() {
            return;
        }
        match &self.rollback {
            ActivationRollback::View(view) => ActiveRetainedHost::restore_appkit(view),
            #[cfg(test)]
            ActivationRollback::TestFlag(flag) => flag.set(false),
        }
    }
}

/// A fully built retained host whose Metal layer is not yet installed on the
/// view. All fallible GPU work is done; [`PreparedRetainedHost::activate`] is the
/// only step that touches the AppKit view.
pub(super) struct PreparedRetainedHost {
    host: RetainedHost,
}

/// A retained host whose Metal layer is installed on the view and rendering.
/// Derefs to the inner [`RetainedHost`] so `render` and the mailbox drain read
/// through transparently.
pub(super) struct ActiveRetainedHost {
    host: RetainedHost,
}

impl PreparedRetainedHost {
    /// Builds the CAMetalLayer, wgpu surface, device, configuration, and
    /// pipelines against a layer that stays detached from the view. A
    /// CAMetalLayer renders fine while detached; installing it on the view later
    /// is what makes it visible. Any failure here leaves the view untouched.
    pub(super) fn prepare(
        view: &NSView,
        mailbox: GpuErrorMailbox,
    ) -> std::result::Result<Self, RetainedFailureCategory> {
        let window = view
            .window()
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        let scale = window.backingScaleFactor();
        let bounds = view.bounds();
        let width = physical_dimension(bounds.size.width, scale);
        let height = physical_dimension(bounds.size.height, scale);
        let layer = unsafe { CAMetalLayer::new() };
        unsafe {
            layer.setDrawableSize(NSSize::new(f64::from(width), f64::from(height)));
        }

        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(descriptor);
        let layer_pointer = std::ptr::from_ref(layer.as_ref())
            .cast_mut()
            .cast::<c_void>();
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_pointer))
        }
        .map_err(|_| RetainedFailureCategory::SurfaceCreate)?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|_| RetainedFailureCategory::AdapterUnavailable)?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glorp-retained-device"),
            ..Default::default()
        }))
        .map_err(|_| RetainedFailureCategory::DeviceUnavailable)?;
        let gpu_error_sender = mailbox.sender();
        device.on_uncaptured_error(Arc::new(move |error| {
            let category = match error {
                wgpu::Error::OutOfMemory { .. } => RetainedFailureCategory::DeviceOutOfMemory,
                wgpu::Error::Validation { .. } => RetainedFailureCategory::DeviceValidation,
                wgpu::Error::Internal { .. } => RetainedFailureCategory::DeviceInternal,
            };
            let _ = gpu_error_sender.send(category);
        }));
        let config = surface
            .get_default_config(&adapter, width, height)
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        surface.configure(&device, &config);
        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glorp-retained-atlas-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipelines = create_pipelines(&device, config.format, &atlas_layout);
        Ok(Self {
            host: RetainedHost {
                surface,
                device,
                queue,
                config,
                layer,
                pipelines,
                atlas_layout,
                glyph_resources: None,
                physical_width: width,
                physical_height: height,
                backing_scale: scale,
                frame_counter: 0,
                gpu_errors: mailbox,
            },
        })
    }

    /// Installs the Metal layer on the view under a rollback guard. This is the
    /// only code that calls `setWantsLayer`/`setLayer`; if a fallible post-attach
    /// step is ever added and fails, the dropped guard restores the view's prior
    /// AppKit layer state before the error propagates.
    pub(super) fn activate(
        self,
        view: &NSView,
    ) -> std::result::Result<ActiveRetainedHost, RetainedFailureCategory> {
        let guard = LayerActivationGuard::install(view);
        view.setWantsLayer(true);
        unsafe { view.setLayer(Some(&self.host.layer)) };
        guard.commit();
        Ok(ActiveRetainedHost { host: self.host })
    }
}

impl ActiveRetainedHost {
    /// Restores the view's prior AppKit layer state. Idempotent, so a redundant
    /// call after fallback is harmless.
    pub(super) fn restore_appkit(view: &NSView) {
        unsafe { view.setLayer(None) };
        view.setWantsLayer(false);
        unsafe { view.setNeedsDisplay(true) };
    }

    /// Renders the frozen paired-review frame into an off-screen intermediate and
    /// reads it back as a [`CanonicalRgbaFrame`]. Reuses the live host's
    /// device/queue/pipelines so the capture rasterizes with the identical
    /// pipeline as the on-screen present.
    pub(crate) fn capture(
        &mut self,
        frame: &crate::companion::paired_review::PairedReviewFrame,
    ) -> std::result::Result<CanonicalRgbaFrame, RetainedFailureCategory> {
        capture::RetainedCaptureTarget::new(&mut self.host).capture(frame)
    }

    /// The physical-pixel drawable size the retained surface is configured for.
    pub(crate) fn physical_size(&self) -> (u32, u32) {
        (self.host.physical_width, self.host.physical_height)
    }

    /// The window backing scale the host resolved its physical size from.
    pub(crate) fn backing_scale(&self) -> f64 {
        self.host.backing_scale
    }

    /// The id of the next frame the host would render. A paired capture stamps
    /// this onto the frozen review frame so both artifacts share one id.
    pub(crate) fn current_frame_id(&self) -> u64 {
        self.host.frame_counter
    }

    /// The resource generation the host currently renders against — the hash of
    /// the active pet's declared content, repertoire, and font policy. Zero before
    /// the first generation is compiled.
    pub(crate) fn current_resource_generation(&self) -> u64 {
        self.host
            .glyph_resources
            .as_ref()
            .map(|active| active.resources.generation().value())
            .unwrap_or(0)
    }
}

impl std::ops::Deref for ActiveRetainedHost {
    type Target = RetainedHost;

    fn deref(&self) -> &Self::Target {
        &self.host
    }
}

impl std::ops::DerefMut for ActiveRetainedHost {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.host
    }
}

impl RetainedHost {
    /// Advances the monotonic frame counter, returning the id for the frame
    /// about to be attempted.
    fn next_frame_id(&mut self) -> u64 {
        let id = self.frame_counter;
        self.frame_counter = self.frame_counter.wrapping_add(1);
        id
    }

    /// Drains any GPU device fault reported asynchronously by the wgpu error
    /// callback. The main thread checks this before treating a present as good.
    pub(super) fn drain_gpu_error(&self) -> Option<RetainedFailureCategory> {
        self.gpu_errors.drain()
    }

    #[allow(clippy::too_many_arguments)] // Explicit prepared-frame inputs keep retained independent of AppState.
    pub(super) fn render(
        &mut self,
        view: &NSView,
        plan: &SmoothCompanionScenePlan,
        draw_order: &[usize],
        metrics: CompanionGridMetrics,
        aperture: RoundAperture,
        background: [f32; 4],
        chrome: RetainedChrome<'_>,
        identity: &CompanionContentIdentity,
    ) -> FrameProgress {
        let frame_id = self.next_frame_id();
        if let Err(category) = self.resize_if_needed(view) {
            let mut progress = FrameProgress::new(frame_id, 0);
            fail(&mut progress, category);
            return progress;
        }
        // Compile the full declared repertoire once per resource generation. A
        // per-frame glyph-set change never rebuilds; only a generation change
        // (species, font policy, or backing scale) does.
        if let Err(category) = self.ensure_resources(identity) {
            let mut progress = FrameProgress::new(frame_id, 0);
            fail(&mut progress, category);
            return progress;
        }
        let Some(active) = self.glyph_resources.as_ref() else {
            let mut progress = FrameProgress::new(frame_id, 0);
            fail(&mut progress, RetainedFailureCategory::AtlasUnavailable);
            return progress;
        };
        let mut progress = FrameProgress::new(frame_id, active.resources.generation().value());
        let atlas = active.resources.atlas();
        let atlas_bind_group = &active.bind_group;
        let frame = match prepare_gpu_frame(
            plan, draw_order, metrics, aperture, background, &chrome, atlas,
        ) {
            Ok(frame) => frame,
            Err(category) => {
                fail(&mut progress, category);
                return progress;
            }
        };
        let primitive_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glorp-retained-primitives"),
                contents: bytemuck::cast_slice(&frame.primitives),
                usage: wgpu::BufferUsages::VERTEX,
            });
        progress
            .mark(FrameMilestone::Prepared)
            .expect("prepared opens the frame ladder");
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                skip(&mut progress, SkipReason::Outdated);
                return progress;
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                skip(&mut progress, SkipReason::Timeout);
                return progress;
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                skip(&mut progress, SkipReason::Occluded);
                return progress;
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                fail(&mut progress, RetainedFailureCategory::SurfaceLost);
                return progress;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                fail(&mut progress, RetainedFailureCategory::SurfaceValidation);
                return progress;
            }
        };
        let target = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glorp-retained-frame"),
            });
        self.encode_scene(
            &mut encoder,
            &target,
            atlas_bind_group,
            &primitive_buffer,
            &frame.blends,
            background,
        );
        progress
            .mark(FrameMilestone::Encoded)
            .expect("encoded follows prepared");
        self.queue.submit([encoder.finish()]);
        progress
            .mark(FrameMilestone::Submitted)
            .expect("submitted follows encoded");
        self.queue.present(surface_texture);
        progress
            .finish(FrameDisposition::SurfacePresentCalled)
            .expect("a submitted frame presents exactly once");
        progress
    }

    /// Encodes the prepared companion scene into `target_view` on `encoder`: one
    /// clear-loaded render pass that draws every primitive with its blend
    /// pipeline. Shared verbatim by the live surface [`Self::render`] path and
    /// the capture intermediate path so both rasterize identical geometry.
    fn encode_scene(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target_view: &wgpu::TextureView,
        atlas_bind_group: &wgpu::BindGroup,
        primitive_buffer: &wgpu::Buffer,
        blends: &[SmoothBlendMode],
        background: [f32; 4],
    ) {
        let attachment = Some(wgpu::RenderPassColorAttachment {
            view: target_view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: f64::from(srgb_channel_to_linear(background[0])),
                    g: f64::from(srgb_channel_to_linear(background[1])),
                    b: f64::from(srgb_channel_to_linear(background[2])),
                    a: f64::from(background[3]),
                }),
                store: wgpu::StoreOp::Store,
            },
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glorp-retained-pass"),
            color_attachments: &[attachment],
            ..Default::default()
        });
        pass.set_bind_group(0, atlas_bind_group, &[]);
        pass.set_vertex_buffer(0, primitive_buffer.slice(..));
        for (index, blend) in blends.iter().copied().enumerate() {
            pass.set_pipeline(self.pipelines.get(blend));
            pass.draw(0..6, index as u32..index as u32 + 1);
        }
    }

    /// Ensures the active glyph resources match the pet's declared-content
    /// `identity` at the current backing scale. Reuses the active generation when
    /// nothing changed; otherwise compiles the full declared repertoire into a
    /// fresh atlas and uploads it. Compile-before-replace: on a compile failure
    /// the previous generation stays active so the caller can keep rendering it or
    /// fall back explicitly.
    fn ensure_resources(
        &mut self,
        identity: &CompanionContentIdentity,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        if let Some(active) = &self.glyph_resources {
            if active.identity == *identity
                && (active.backing_scale - self.backing_scale).abs() < f64::EPSILON
            {
                return Ok(());
            }
        }
        // The first compile is the activation build; any later one is a
        // legitimate, rare rebuild a resource generation change (e.g. a resize's
        // backing-scale change) caused — never per-frame animation churn.
        let manifest =
            GlyphRepertoireManifest::for_active_pet(identity.clone(), self.backing_scale);
        let resources = CompiledRetainedResources::compile(&manifest)?;
        let (texture, bind_group) = self.upload_glyph_atlas(resources.atlas());
        self.glyph_resources = Some(ActiveGlyphResources {
            identity: identity.clone(),
            backing_scale: self.backing_scale,
            resources,
            _texture: texture,
            bind_group,
        });
        Ok(())
    }

    /// Uploads a compiled glyph atlas into a GPU texture and builds its bind
    /// group.
    fn upload_glyph_atlas(&self, atlas: &CompiledGlyphAtlas) -> (wgpu::Texture, wgpu::BindGroup) {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glorp-retained-glyph-atlas"),
            size: wgpu::Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atlas.width * 4),
                rows_per_image: Some(atlas.height),
            },
            wgpu::Extent3d {
                width: atlas.width,
                height: atlas.height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glorp-retained-atlas-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glorp-retained-atlas-bind-group"),
            layout: &self.atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        (texture, bind_group)
    }

    fn resize_if_needed(
        &mut self,
        view: &NSView,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        let window = view
            .window()
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        let scale = window.backingScaleFactor();
        let bounds = view.bounds();
        let width = physical_dimension(bounds.size.width, scale);
        let height = physical_dimension(bounds.size.height, scale);
        if width == self.physical_width
            && height == self.physical_height
            && (scale - self.backing_scale).abs() < f64::EPSILON
        {
            return Ok(());
        }
        self.physical_width = width;
        self.physical_height = height;
        self.backing_scale = scale;
        self.config.width = width;
        self.config.height = height;
        unsafe {
            self.layer
                .setDrawableSize(NSSize::new(f64::from(width), f64::from(height)))
        };
        self.surface.configure(&self.device, &self.config);
        Ok(())
    }
}

/// Terminates a frame that could not present because a render resource failed.
fn fail(progress: &mut FrameProgress, category: RetainedFailureCategory) {
    progress
        .finish(FrameDisposition::Failed(category))
        .expect("a frame reaches its terminal disposition exactly once");
}

/// Terminates a frame the surface asked us to drop this tick without failing.
fn skip(progress: &mut FrameProgress, reason: SkipReason) {
    progress
        .finish(FrameDisposition::Skipped(reason))
        .expect("a frame reaches its terminal disposition exactly once");
}

fn physical_dimension(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().clamp(1.0, f64::from(u32::MAX)) as u32
}

fn create_pipelines(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    atlas_layout: &wgpu::BindGroupLayout,
) -> Pipelines {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glorp-retained-shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("retained.wgsl").into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glorp-retained-pipeline-layout"),
        bind_group_layouts: &[Some(atlas_layout)],
        immediate_size: 0,
    });
    let create = |label: &'static str, blend: Option<wgpu::BlendState>| {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(GpuPrimitive::LAYOUT)],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    };
    let normal = Some(wgpu::BlendState::ALPHA_BLENDING);
    let multiply = Some(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Dst,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    });
    let screen = Some(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrc,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    });
    let add = Some(wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent::OVER,
    });
    Pipelines {
        normal: create("glorp-retained-normal", normal),
        multiply: create("glorp-retained-multiply", multiply),
        screen: create("glorp-retained-screen", screen),
        add: create("glorp-retained-add", add),
        replace: create("glorp-retained-replace", None),
    }
}

fn prepare_gpu_frame(
    plan: &SmoothCompanionScenePlan,
    draw_order: &[usize],
    metrics: CompanionGridMetrics,
    aperture: RoundAperture,
    background: [f32; 4],
    chrome: &RetainedChrome<'_>,
    atlas: &CompiledGlyphAtlas,
) -> std::result::Result<PreparedGpuFrame, RetainedFailureCategory> {
    let mut primitives = Vec::new();
    let mut blends = Vec::new();
    let viewport_aperture = viewport_aperture(aperture);
    let aperture_radius = aperture_radius(aperture);
    push_tank_background(
        &mut primitives,
        &mut blends,
        aperture,
        background,
        viewport_aperture,
        aperture_radius,
    );
    push_mood_aura(
        &mut primitives,
        &mut blends,
        metrics,
        aperture,
        chrome,
        viewport_aperture,
        aperture_radius,
    );
    for &index in draw_order {
        let Some(layer) = plan.layers.get(index) else {
            continue;
        };
        if layer.opacity <= 0.0 {
            continue;
        }
        let blend_code = match layer.blend {
            SmoothBlendMode::Multiply => 1.0,
            SmoothBlendMode::Screen => 2.0,
            _ => 0.0,
        };
        for item in &layer.items {
            match item {
                SmoothLayerItem::LocalCell(cell) => {
                    let rect = cell_rect(layer, cell.col, cell.row, metrics);
                    let (clip_rect, clip_ellipse, clip_kind) = clip_params(layer.clip, metrics);
                    if let Some(bg) = cell.bg {
                        primitives.push(GpuPrimitive {
                            rect,
                            color_a: rgb(bg.r, bg.g, bg.b, layer.opacity),
                            color_b: [0.0; 4],
                            uv: [0.0; 4],
                            params: [1.0, clip_kind, blend_code, 0.0],
                            clip_rect,
                            clip_ellipse,
                            viewport_aperture,
                            aperture_radius,
                        });
                        blends.push(layer.blend);
                    }
                    if let Some(glyph) = cell.glyph.as_ref() {
                        let entry = atlas
                            .entries
                            .get(&GlyphKey::new(glyph.clone(), cell.bold))
                            .copied()
                            .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
                        let fg = cell.fg.map_or([1.0, 1.0, 1.0, layer.opacity], |fg| {
                            rgb(fg.r, fg.g, fg.b, layer.opacity)
                        });
                        if let Some(glyph_rect) = glyph_ink_rect(
                            [rect[0], rect[1]],
                            metrics.font_size as f32 * layer.transform.scale.x,
                            entry,
                        ) {
                            let uv = entry
                                .visible_uv
                                .expect("a visible ink rect implies a visible uv");
                            primitives.push(GpuPrimitive {
                                rect: glyph_rect,
                                color_a: fg,
                                color_b: [0.0; 4],
                                uv,
                                params: [0.0, clip_kind, blend_code, glyph_mode_flag(entry)],
                                clip_rect,
                                clip_ellipse,
                                viewport_aperture,
                                aperture_radius,
                            });
                            blends.push(layer.blend);
                        }
                    }
                }
                SmoothLayerItem::Shape(shape) => {
                    let SmoothShapeGeometry::Ellipse { bounds } = shape.geometry;
                    let rect = shape_rect(layer, bounds.min, bounds.max, metrics);
                    let (color_a, color_b, kind) = match shape.fill {
                        SmoothFill::Solid(color) => (rgba(color, layer.opacity), [0.0; 4], 2.0),
                        SmoothFill::RadialGradient { inner, outer } => {
                            (rgba(inner, layer.opacity), rgba(outer, layer.opacity), 3.0)
                        }
                        SmoothFill::LinearGradientY { top, bottom } => {
                            (rgba(top, layer.opacity), rgba(bottom, layer.opacity), 4.0)
                        }
                    };
                    let (clip_rect, clip_ellipse, clip_kind) = clip_params(layer.clip, metrics);
                    primitives.push(GpuPrimitive {
                        rect,
                        color_a,
                        color_b,
                        uv: [0.0; 4],
                        params: [kind, clip_kind, blend_code, 0.0],
                        clip_rect,
                        clip_ellipse,
                        viewport_aperture,
                        aperture_radius,
                    });
                    blends.push(layer.blend);
                }
                SmoothLayerItem::Raster(_) => {
                    return Err(RetainedFailureCategory::UnsupportedRaster)
                }
            }
        }
    }
    push_gauges(
        &mut primitives,
        &mut blends,
        aperture,
        chrome.gauges,
        viewport_aperture,
        aperture_radius,
    );
    push_overlays(
        &mut primitives,
        &mut blends,
        chrome.overlays,
        viewport_aperture,
        aperture_radius,
    );
    push_hud(
        &mut primitives,
        &mut blends,
        atlas,
        aperture,
        chrome.hud,
        chrome.hud_font_size,
        viewport_aperture,
        aperture_radius,
    )?;
    if chrome.dim_overlay {
        push_solid_primitive(
            &mut primitives,
            &mut blends,
            [0.0, 0.0, aperture.width as f32, aperture.height as f32],
            display_rgba([0.05, 0.06, 0.10, 0.35]),
            1.0,
            viewport_aperture,
            aperture_radius,
        );
    }
    Ok(PreparedGpuFrame { primitives, blends })
}

fn viewport_aperture(aperture: RoundAperture) -> [f32; 4] {
    [
        aperture.width as f32,
        aperture.height as f32,
        aperture.center_x,
        aperture.center_y,
    ]
}

fn aperture_radius(aperture: RoundAperture) -> [f32; 4] {
    [aperture.radius, 0.0, 0.0, 0.0]
}

fn push_tank_background(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    aperture: RoundAperture,
    background: [f32; 4],
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let mix = |base: f32, tint: f32| base + (tint - base) * TANK_CORE_TINT_WEIGHT;
    let core = [
        mix(background[0], TANK_CORE_TINT[0]),
        mix(background[1], TANK_CORE_TINT[1]),
        mix(background[2], TANK_CORE_TINT[2]),
        background[3],
    ];
    primitives.push(GpuPrimitive {
        rect: [
            aperture.center_x - aperture.radius,
            aperture.center_y - aperture.radius,
            aperture.radius * 2.0,
            aperture.radius * 2.0,
        ],
        color_a: display_rgba(core),
        color_b: display_rgba(background),
        uv: [0.0; 4],
        // Kind 3 reproduces Smooth's output-level radial dither in the shader.
        params: [3.0, 0.0, 0.0, 0.0],
        clip_rect: [0.0; 4],
        clip_ellipse: [0.0; 4],
        viewport_aperture,
        aperture_radius,
    });
    blends.push(SmoothBlendMode::Replace);
}

fn push_mood_aura(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    metrics: CompanionGridMetrics,
    aperture: RoundAperture,
    chrome: &RetainedChrome<'_>,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let center_x = metrics.origin_x + chrome.pet_center_col * metrics.cell_w;
    let center_y = metrics.origin_y - (chrome.pet_center_row + 1.0) * metrics.cell_h;
    let max_radius = chrome.pet_width_cells * metrics.cell_w * 0.95;
    for index in 0..8 {
        let t = index as f64 / 8.0;
        let radius = max_radius * (1.0 - t);
        push_solid_primitive(
            primitives,
            blends,
            [
                (center_x - radius) as f32,
                (center_y - radius) as f32,
                (radius * 2.0) as f32,
                (radius * 2.0) as f32,
            ],
            display_rgba([
                chrome.mood_aura[0],
                chrome.mood_aura[1],
                chrome.mood_aura[2],
                0.05,
            ]),
            2.0,
            viewport_aperture,
            aperture_radius,
        );
    }
    let _ = aperture;
}

fn push_gauges(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    aperture: RoundAperture,
    gauges: PreparedGaugeFrame,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let layout = perimeter_gauge_layout(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        f64::from(aperture.radius),
        COMPANION_GAUGE_GAP_DEG,
    );
    let colors = perimeter_gauge_colors();
    push_gauge_lane(
        primitives,
        blends,
        &layout.xp,
        &colors.xp,
        gauges.xp_fraction,
        viewport_aperture,
        aperture_radius,
    );
    push_gauge_lane(
        primitives,
        blends,
        &layout.daily,
        &colors.daily,
        gauges.daily_fraction,
        viewport_aperture,
        aperture_radius,
    );
    push_gauge_arc(
        primitives,
        blends,
        &layout.daily,
        daily_overage_color(),
        gauges.daily_overage_fraction,
        viewport_aperture,
        aperture_radius,
    );
    push_gauge_lane(
        primitives,
        blends,
        &layout.pace,
        &colors.pace,
        gauges.pace_fraction,
        viewport_aperture,
        aperture_radius,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_gauge_lane(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    lane: &GaugeLane,
    colors: &GaugeLaneColors,
    fraction: f64,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    push_gauge_arc(
        primitives,
        blends,
        lane,
        colors.track,
        1.0,
        viewport_aperture,
        aperture_radius,
    );
    push_gauge_arc(
        primitives,
        blends,
        lane,
        colors.fill,
        fraction,
        viewport_aperture,
        aperture_radius,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_gauge_arc(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    lane: &GaugeLane,
    color: RoundColor,
    fraction: f64,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let fraction = fraction.clamp(0.0, 1.0);
    if fraction <= 0.0 {
        return;
    }
    let start_deg = lane.ring.track_start_deg;
    let end_deg = growth_ring_fill_end_deg(&lane.ring, fraction);
    push_analytic_arc(
        primitives,
        blends,
        lane,
        color,
        start_deg,
        end_deg,
        viewport_aperture,
        aperture_radius,
    );
}

#[allow(clippy::too_many_arguments)]
fn push_analytic_arc(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    lane: &GaugeLane,
    color: RoundColor,
    start_deg: f64,
    end_deg: f64,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    let outer_radius = (lane.ring.radius + lane.stroke_width / 2.0) as f32;
    if outer_radius <= 0.0 || end_deg <= start_deg {
        return;
    }
    primitives.push(GpuPrimitive {
        rect: [
            lane.ring.cx as f32 - outer_radius,
            lane.ring.cy as f32 - outer_radius,
            outer_radius * 2.0,
            outer_radius * 2.0,
        ],
        color_a: display_round_color(color),
        color_b: [0.0; 4],
        // Analytic arc parameters: start/sweep radians, centerline and half-width
        // normalized by the primitive's outer radius.
        uv: [
            start_deg.to_radians() as f32,
            (end_deg - start_deg).to_radians() as f32,
            lane.ring.radius as f32 / outer_radius,
            lane.stroke_width as f32 / (2.0 * outer_radius),
        ],
        params: [
            5.0,
            0.0,
            0.0,
            if lane.cap == LineCap::Round { 1.0 } else { 0.0 },
        ],
        clip_rect: [0.0; 4],
        clip_ellipse: [0.0; 4],
        viewport_aperture,
        aperture_radius,
    });
    blends.push(SmoothBlendMode::Normal);
}

fn push_overlays(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    overlays: &[RoundDrawCommand],
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    for command in overlays
        .iter()
        .filter(|command| matches!(command.kind, RoundDrawKind::Halo | RoundDrawKind::Trouble))
    {
        push_solid_primitive(
            primitives,
            blends,
            [
                command.x - command.radius,
                command.y - command.radius,
                command.radius * 2.0,
                command.radius * 2.0,
            ],
            display_round_color(command.color),
            2.0,
            viewport_aperture,
            aperture_radius,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_hud(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    atlas: &CompiledGlyphAtlas,
    aperture: RoundAperture,
    hud: &CompanionHudText,
    hud_font_size: f64,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) -> std::result::Result<(), RetainedFailureCategory> {
    let gauge_layout = perimeter_gauge_layout(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        f64::from(aperture.radius),
        COMPANION_GAUGE_GAP_DEG,
    );
    let gap = stat_gap_box(
        f64::from(aperture.center_x),
        f64::from(aperture.center_y),
        gauge_layout.pace.ring.radius - gauge_layout.pace.stroke_width / 2.0,
        COMPANION_GAUGE_GAP_DEG,
    );
    let mut stack_size = hud_font_size * 1.45;
    let big_color = display_rgba([0.93, 0.93, 0.97, 1.0]);
    let sub_color = display_rgba([0.62, 0.63, 0.77, 1.0]);
    let mut layout = hud_layout(atlas, hud, stack_size as f32);
    while (f64::from(layout.max_width) > gap.max_width
        || f64::from(layout.total_height) > f64::from(aperture.radius) * 0.34)
        && stack_size > 6.0
    {
        stack_size -= 1.0;
        layout = hud_layout(atlas, hud, stack_size as f32);
    }
    let lines = [
        (&hud.today_total, layout.lines[0], big_color),
        (&hud.daily_percent, layout.lines[1], sub_color),
        (&hud.pace, layout.lines[2], sub_color),
    ];
    let top = f64::from(aperture.height) - gap.baseline_y;
    let mut y = top + f64::from(layout.total_height) * 0.38;
    for (line, line_layout, color) in lines {
        debug_assert!(line.is_ascii(), "HUD text is ASCII by contract: {line:?}");
        let width = f64::from(line_layout.width);
        let mut x = gap.center_x - width / 2.0;
        for scalar in line.chars() {
            let entry = atlas
                .entries
                .get(&GlyphKey::new(scalar.to_string(), false))
                .copied()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            if let Some(rect) = glyph_ink_rect([x as f32, y as f32], line_layout.font_size, entry) {
                let uv = entry
                    .visible_uv
                    .expect("a visible ink rect implies a visible uv");
                primitives.push(GpuPrimitive {
                    rect,
                    color_a: color,
                    color_b: [0.0; 4],
                    uv,
                    params: [0.0, 0.0, 0.0, glyph_mode_flag(entry)],
                    clip_rect: [0.0; 4],
                    clip_ellipse: [0.0; 4],
                    viewport_aperture,
                    aperture_radius,
                });
                blends.push(SmoothBlendMode::Normal);
            }
            x += f64::from(glyph_advance(entry, line_layout.font_size));
        }
        y -= f64::from(line_layout.height * 0.82);
    }
    Ok(())
}

/// The glyph-mode flag a glyph primitive carries in `params[3]`: 0.0 for a
/// coverage mask (tinted by the authored color), 1.0 for native color (the
/// shader samples the premultiplied RGBA and bypasses the tint).
fn glyph_mode_flag(entry: GlyphAtlasEntry) -> f32 {
    match entry.fragment_mode() {
        FragmentGlyphMode::Mask => 0.0,
        FragmentGlyphMode::NativeColor => 1.0,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HudLineLayout {
    font_size: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct HudLayout {
    lines: [HudLineLayout; 3],
    max_width: f32,
    total_height: f32,
}

fn hud_layout(atlas: &CompiledGlyphAtlas, hud: &CompanionHudText, stack_size: f32) -> HudLayout {
    let lines = [
        hud_line_layout(atlas, &hud.today_total, stack_size * 1.08),
        hud_line_layout(atlas, &hud.daily_percent, stack_size * 0.68),
        hud_line_layout(atlas, &hud.pace, stack_size * 0.68),
    ];
    HudLayout {
        max_width: lines.iter().map(|line| line.width).fold(0.0, f32::max),
        total_height: lines[0].height + lines[1].height * 0.82 + lines[2].height * 0.82,
        lines,
    }
}

fn hud_line_layout(atlas: &CompiledGlyphAtlas, text: &str, font_size: f32) -> HudLineLayout {
    HudLineLayout {
        font_size,
        width: glyph_run_width(atlas, text, font_size),
        height: glyph_run_height(atlas, text, font_size),
    }
}

#[allow(clippy::too_many_arguments)]
fn push_solid_primitive(
    primitives: &mut Vec<GpuPrimitive>,
    blends: &mut Vec<SmoothBlendMode>,
    rect: [f32; 4],
    color: [f32; 4],
    kind: f32,
    viewport_aperture: [f32; 4],
    aperture_radius: [f32; 4],
) {
    primitives.push(GpuPrimitive {
        rect,
        color_a: color,
        color_b: [0.0; 4],
        uv: [0.0; 4],
        params: [kind, 0.0, 0.0, 0.0],
        clip_rect: [0.0; 4],
        clip_ellipse: [0.0; 4],
        viewport_aperture,
        aperture_radius,
    });
    blends.push(SmoothBlendMode::Normal);
}

fn display_round_color(color: RoundColor) -> [f32; 4] {
    display_rgba([color.0, color.1, color.2, color.3])
}

fn display_rgba(color: [f32; 4]) -> [f32; 4] {
    [
        srgb_channel_to_linear(color[0]),
        srgb_channel_to_linear(color[1]),
        srgb_channel_to_linear(color[2]),
        color[3],
    ]
}

fn glyph_scale(font_size: f32) -> f32 {
    font_size / GLYPH_FONT_SIZE as f32
}

fn glyph_advance(entry: GlyphAtlasEntry, font_size: f32) -> f32 {
    entry.advance * glyph_scale(font_size)
}

fn glyph_run_width(atlas: &CompiledGlyphAtlas, text: &str, font_size: f32) -> f32 {
    debug_assert!(text.is_ascii(), "HUD text is ASCII by contract: {text:?}");
    text.chars()
        .filter_map(|scalar| {
            atlas
                .entries
                .get(&GlyphKey::new(scalar.to_string(), false))
                .copied()
        })
        .map(|entry| glyph_advance(entry, font_size))
        .sum()
}

fn glyph_run_height(atlas: &CompiledGlyphAtlas, text: &str, font_size: f32) -> f32 {
    debug_assert!(text.is_ascii(), "HUD text is ASCII by contract: {text:?}");
    text.chars()
        .filter_map(|scalar| {
            atlas
                .entries
                .get(&GlyphKey::new(scalar.to_string(), false))
                .copied()
        })
        .map(|entry| entry.line_height * glyph_scale(font_size))
        .fold(0.0, f32::max)
}

fn glyph_ink_rect(
    draw_origin: [f32; 2],
    font_size: f32,
    entry: GlyphAtlasEntry,
) -> Option<[f32; 4]> {
    if entry.ink_size[0] <= 0.0 || entry.ink_size[1] <= 0.0 {
        return None;
    }
    let scale = glyph_scale(font_size);
    Some([
        draw_origin[0] + entry.ink_origin[0] * scale,
        draw_origin[1] + entry.ink_origin[1] * scale,
        entry.ink_size[0] * scale,
        entry.ink_size[1] * scale,
    ])
}

fn layer_point(layer: &SmoothCompanionLayer, local: SmoothPoint) -> SmoothPoint {
    let pivot = SmoothPoint {
        x: layer.anchor.x + layer.transform_origin.x,
        y: layer.anchor.y + layer.transform_origin.y,
    };
    SmoothPoint {
        x: pivot.x
            + (layer.anchor.x + local.x - pivot.x) * layer.transform.scale.x
            + layer.transform.translation.x,
        y: pivot.y
            + (layer.anchor.y + local.y - pivot.y) * layer.transform.scale.y
            + layer.transform.translation.y,
    }
}

fn cell_rect(
    layer: &SmoothCompanionLayer,
    col: u16,
    row: u16,
    metrics: CompanionGridMetrics,
) -> [f32; 4] {
    let p = layer_point(layer, SmoothPoint { x: f32::from(col), y: f32::from(row) });
    let scale = layer.transform.scale.x;
    let fractional = matches!(
        layer.motion_binding,
        SmoothLayerMotionBinding::PetAttached
            | SmoothLayerMotionBinding::FloorProjected
            | SmoothLayerMotionBinding::Parallax(_)
    );
    let x = metrics.origin_x as f32 + p.x * metrics.cell_w as f32;
    let y = if fractional {
        metrics.origin_y as f32 - (p.y + scale) * metrics.cell_h as f32
    } else {
        metrics.origin_y as f32 - (p.y.round() + 1.0) * metrics.cell_h as f32
    };
    [
        x,
        y,
        metrics.cell_w as f32 * scale,
        metrics.cell_h as f32 * scale,
    ]
}

fn shape_rect(
    layer: &SmoothCompanionLayer,
    min: SmoothPoint,
    max: SmoothPoint,
    metrics: CompanionGridMetrics,
) -> [f32; 4] {
    let min = layer_point(layer, min);
    let max = layer_point(layer, max);
    [
        metrics.origin_x as f32 + min.x * metrics.cell_w as f32,
        metrics.origin_y as f32 - max.y * metrics.cell_h as f32,
        (max.x - min.x) * metrics.cell_w as f32,
        (max.y - min.y) * metrics.cell_h as f32,
    ]
}

fn clip_params(clip: SmoothClip, metrics: CompanionGridMetrics) -> ([f32; 4], [f32; 4], f32) {
    match clip {
        SmoothClip::None => ([0.0; 4], [0.0; 4], 0.0),
        SmoothClip::Rect(bounds) => (
            [
                metrics.origin_x as f32 + bounds.min.x * metrics.cell_w as f32,
                metrics.origin_y as f32 - bounds.max.y * metrics.cell_h as f32,
                (bounds.max.x - bounds.min.x) * metrics.cell_w as f32,
                (bounds.max.y - bounds.min.y) * metrics.cell_h as f32,
            ],
            [0.0; 4],
            1.0,
        ),
        SmoothClip::Circle { center, radius } => (
            [0.0; 4],
            [
                metrics.origin_x as f32 + center.x * metrics.cell_w as f32,
                metrics.origin_y as f32 - center.y * metrics.cell_h as f32,
                radius * metrics.cell_w as f32,
                radius * metrics.cell_h as f32,
            ],
            2.0,
        ),
        SmoothClip::Ellipse { center, radii } => (
            [0.0; 4],
            [
                metrics.origin_x as f32 + center.x * metrics.cell_w as f32,
                metrics.origin_y as f32 - center.y * metrics.cell_h as f32,
                radii.x * metrics.cell_w as f32,
                radii.y * metrics.cell_h as f32,
            ],
            2.0,
        ),
    }
}

fn rgba(color: SmoothRgba8, opacity: f32) -> [f32; 4] {
    [
        srgb_channel_to_linear(color.r as f32 / 255.0),
        srgb_channel_to_linear(color.g as f32 / 255.0),
        srgb_channel_to_linear(color.b as f32 / 255.0),
        color.a as f32 / 255.0 * opacity,
    ]
}

fn rgb(r: u8, g: u8, b: u8, opacity: f32) -> [f32; 4] {
    [
        srgb_channel_to_linear(r as f32 / 255.0),
        srgb_channel_to_linear(g as f32 / 255.0),
        srgb_channel_to_linear(b as f32 / 255.0),
        opacity,
    ]
}

fn srgb_channel_to_linear(channel: f32) -> f32 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::resources::GlyphEntryKind;
    use super::{
        glyph_advance, glyph_ink_rect, hud_layout, physical_dimension, push_analytic_arc,
        srgb_channel_to_linear, CompiledGlyphAtlas, GlyphAtlasEntry, GlyphKey,
        GlyphRepertoireManifest, LayerActivationGuard, LayerActivationState,
    };
    use crate::round::smooth::CompanionContentIdentity;
    use std::collections::BTreeMap;

    /// A visible coverage-mask entry with the given ink geometry; the metric
    /// fields the retained layout does not read stay neutral.
    fn mask_entry(
        visible_uv: Option<[f32; 4]>,
        ink_origin: [f32; 2],
        ink_size: [f32; 2],
        advance: f32,
        line_height: f32,
    ) -> GlyphAtlasEntry {
        GlyphAtlasEntry {
            visible_uv,
            ink_origin,
            ink_size,
            baseline: 0.0,
            ascent: 0.0,
            descent: 0.0,
            line_height,
            advance,
            raster_size: [80.0, 80.0],
            safe_padding: 6.0,
            font_policy_id: 0,
            kind: GlyphEntryKind::Mask,
        }
    }

    #[test]
    fn failed_preflight_never_marks_layer_attached() {
        let mut state = LayerActivationState::default();
        state.preflight_failed();
        assert!(!state.attached());
        assert!(state.appkit_restored());
    }

    #[test]
    fn activation_guard_restores_uncommitted_attachment() {
        let state = std::rc::Rc::new(std::cell::Cell::new(true));
        {
            let _guard = LayerActivationGuard::for_test(state.clone());
        }
        assert!(!state.get());
    }

    #[test]
    fn physical_dimensions_follow_backing_scale() {
        assert_eq!(physical_dimension(360.0, 2.0), 720);
        assert_eq!(physical_dimension(0.0, 2.0), 1);
    }

    #[test]
    fn srgb_colors_are_linearized_for_an_srgb_surface() {
        assert_eq!(srgb_channel_to_linear(0.0), 0.0);
        assert_eq!(srgb_channel_to_linear(1.0), 1.0);
        assert!((srgb_channel_to_linear(0.5) - 0.214_041_14).abs() < 0.000_001);
    }

    #[test]
    fn glyph_quad_uses_ink_bounds_instead_of_filling_the_cell() {
        let entry = mask_entry(
            Some([0.1, 0.2, 0.3, 0.4]),
            [3.0, 7.0],
            [20.0, 31.0],
            29.0,
            52.0,
        );

        assert_eq!(
            glyph_ink_rect([100.0, 200.0], 24.0, entry),
            Some([101.5, 203.5, 10.0, 15.5])
        );
        assert_eq!(glyph_advance(entry, 24.0), 14.5);
    }

    #[test]
    fn blank_glyph_has_advance_without_a_visible_quad() {
        let entry = GlyphAtlasEntry::whitespace(28.0, 52.0);

        assert_eq!(glyph_ink_rect([12.0, 34.0], 48.0, entry), None);
        assert_eq!(glyph_advance(entry, 48.0), 28.0);
    }

    #[test]
    fn atlas_generation_keys_on_declared_content_not_the_per_frame_glyph_set() {
        use crate::pet::generation::Species;
        let crystal = || {
            GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Crystal),
                2.0,
            )
            .generation_key()
        };
        // Same declared content -> same generation (the per-minute room reshuffle
        // changes which glyph is painted, never the identity, so no rebuild).
        assert_eq!(crystal(), crystal());
        // A different species is a real generation change.
        assert_ne!(
            crystal(),
            GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Mech),
                2.0,
            )
            .generation_key(),
        );
        // A backing-scale change is a real generation change (Task-8 finding #3).
        assert_ne!(
            crystal(),
            GlyphRepertoireManifest::for_active_pet(
                CompanionContentIdentity::for_pet(Species::Crystal),
                1.0,
            )
            .generation_key(),
        );
    }

    #[test]
    fn hud_layout_uses_the_same_big_and_subline_font_policy_as_smooth() {
        let entry = mask_entry(Some([0.0; 4]), [0.0; 2], [10.0, 20.0], 30.0, 50.0);
        let atlas = CompiledGlyphAtlas {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
            entries: [GlyphKey::new("A", false), GlyphKey::new("B", false)]
                .into_iter()
                .map(|key| (key, entry))
                .collect::<BTreeMap<_, _>>(),
        };
        let hud = crate::round::hud::CompanionHudText {
            today_total: "AA".into(),
            daily_percent: "A".into(),
            pace: "BB".into(),
        };

        let layout = hud_layout(&atlas, &hud, 20.0);

        assert!((layout.lines[0].font_size - 21.6).abs() < 0.001);
        assert!((layout.lines[1].font_size - 13.6).abs() < 0.001);
        assert_eq!(layout.lines[1].font_size, layout.lines[2].font_size);
        assert!(layout.lines[0].height > layout.lines[1].height);
        assert_eq!(
            layout.max_width,
            layout.lines[0].width.max(layout.lines[2].width)
        );
    }

    #[test]
    fn gauge_arc_is_one_analytic_primitive_with_exact_shared_angles_and_round_cap() {
        let lane = crate::round::hud::perimeter_gauge_layout(
            180.0,
            180.0,
            180.0,
            crate::round::hud::COMPANION_GAUGE_GAP_DEG,
        )
        .xp;
        let mut primitives = Vec::new();
        let mut blends = Vec::new();
        push_analytic_arc(
            &mut primitives,
            &mut blends,
            &lane,
            crate::round::draw::RoundColor(1.0, 0.0, 0.0, 1.0),
            lane.ring.track_start_deg,
            crate::round::hud::growth_ring_fill_end_deg(&lane.ring, 0.25),
            [360.0, 360.0, 180.0, 180.0],
            [180.0, 0.0, 0.0, 0.0],
        );

        assert_eq!(primitives.len(), 1);
        assert_eq!(
            blends,
            vec![crate::presentation::smooth::SmoothBlendMode::Normal]
        );
        let primitive = primitives[0];
        assert_eq!(primitive.params[0], 5.0);
        assert_eq!(primitive.params[3], 1.0);
        assert!((primitive.uv[0] - lane.ring.track_start_deg.to_radians() as f32).abs() < 1e-6);
        assert!(
            (primitive.uv[1] - (lane.ring.track_sweep_deg * 0.25).to_radians() as f32).abs() < 1e-6
        );
    }
}
