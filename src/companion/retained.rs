#![cfg(all(target_os = "macos", feature = "retained-renderer"))]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::c_void;
use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use objc2::rc::Retained;
use objc2::ClassType;
use objc2_app_kit::{
    NSAttributedStringNSStringDrawing, NSBitmapImageRep, NSColor, NSDeviceRGBColorSpace, NSFont,
    NSFontAttributeName, NSFontWeightBold, NSForegroundColorAttributeName, NSGraphicsContext,
    NSView,
};
use objc2_foundation::{NSMutableAttributedString, NSPoint, NSRange, NSSize, NSString};
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

mod presentation;

pub(crate) use presentation::{
    FrameDisposition, FrameProgress, RetainedFailureCategory, SkipReason,
};
use presentation::{FrameMilestone, GpuErrorMailbox};

const ATLAS_CELL: u32 = 80;
const ATLAS_PADDING: u32 = 6;
const ATLAS_COLUMNS: u32 = 16;
const MAX_ATLAS_ROWS: u32 = 16;
const GLYPH_FONT_SIZE: f64 = 48.0;
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

#[derive(Debug, Clone, Copy)]
struct GlyphAtlasEntry {
    uv: [f32; 4],
    ink_origin: [f32; 2],
    ink_size: [f32; 2],
    advance: f32,
    line_height: f32,
}

struct GlyphAtlas {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    entries: BTreeMap<(String, bool), GlyphAtlasEntry>,
}

struct CachedGlyphAtlas {
    glyphs: Vec<(String, bool)>,
    atlas: GlyphAtlas,
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
    glyph_atlas: Option<CachedGlyphAtlas>,
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

impl RetainedHost {
    pub(super) fn new(view: &NSView) -> std::result::Result<Self, RetainedFailureCategory> {
        let window = view
            .window()
            .ok_or(RetainedFailureCategory::SurfaceUnavailable)?;
        let scale = window.backingScaleFactor();
        let bounds = view.bounds();
        let width = physical_dimension(bounds.size.width, scale);
        let height = physical_dimension(bounds.size.height, scale);
        let layer = unsafe { CAMetalLayer::new() };
        view.setWantsLayer(true);
        unsafe {
            layer.setDrawableSize(NSSize::new(f64::from(width), f64::from(height)));
            view.setLayer(Some(&layer));
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
        let gpu_errors = GpuErrorMailbox::new();
        let gpu_error_sender = gpu_errors.sender();
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
            surface,
            device,
            queue,
            config,
            layer,
            pipelines,
            atlas_layout,
            glyph_atlas: None,
            physical_width: width,
            physical_height: height,
            backing_scale: scale,
            frame_counter: 0,
            gpu_errors,
        })
    }

    pub(super) fn restore_appkit(view: &NSView) {
        unsafe { view.setLayer(None) };
        view.setWantsLayer(false);
        unsafe { view.setNeedsDisplay(true) };
    }

    /// Advances the monotonic frame counter, returning the id for the frame
    /// about to be attempted. Real resource generations arrive in Task 9.
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
    ) -> FrameProgress {
        let mut progress = FrameProgress::new(self.next_frame_id(), 0);
        if let Err(category) = self.resize_if_needed(view) {
            fail(&mut progress, category);
            return progress;
        }
        let glyphs = collect_glyphs(plan, chrome.hud);
        if let Err(category) = self.ensure_glyph_atlas(glyphs) {
            fail(&mut progress, category);
            return progress;
        }
        let Some(cached_atlas) = self.glyph_atlas.as_ref() else {
            fail(&mut progress, RetainedFailureCategory::AtlasUnavailable);
            return progress;
        };
        let frame = match prepare_gpu_frame(
            plan,
            draw_order,
            metrics,
            aperture,
            background,
            &chrome,
            &cached_atlas.atlas,
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
        {
            let attachment = Some(wgpu::RenderPassColorAttachment {
                view: &target,
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
            pass.set_bind_group(0, &cached_atlas.bind_group, &[]);
            pass.set_vertex_buffer(0, primitive_buffer.slice(..));
            for (index, blend) in frame.blends.iter().copied().enumerate() {
                pass.set_pipeline(self.pipelines.get(blend));
                pass.draw(0..6, index as u32..index as u32 + 1);
            }
        }
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

    fn ensure_glyph_atlas(
        &mut self,
        glyphs: Vec<(String, bool)>,
    ) -> std::result::Result<(), RetainedFailureCategory> {
        if !glyph_atlas_needs_rebuild(
            self.glyph_atlas.as_ref().map(|cached| &cached.glyphs),
            &glyphs,
        ) {
            return Ok(());
        }

        let atlas = build_glyph_atlas(&glyphs)?;
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
        self.glyph_atlas = Some(CachedGlyphAtlas {
            glyphs,
            atlas,
            _texture: texture,
            bind_group,
        });
        Ok(())
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
    atlas: &GlyphAtlas,
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
                            .get(&(glyph.clone(), cell.bold))
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
                            primitives.push(GpuPrimitive {
                                rect: glyph_rect,
                                color_a: fg,
                                color_b: [0.0; 4],
                                uv: entry.uv,
                                params: [0.0, clip_kind, blend_code, 0.0],
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
    atlas: &GlyphAtlas,
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
        let width = f64::from(line_layout.width);
        let mut x = gap.center_x - width / 2.0;
        for glyph in line.chars() {
            let entry = atlas
                .entries
                .get(&(glyph.to_string(), false))
                .copied()
                .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
            if let Some(rect) = glyph_ink_rect([x as f32, y as f32], line_layout.font_size, entry) {
                primitives.push(GpuPrimitive {
                    rect,
                    color_a: color,
                    color_b: [0.0; 4],
                    uv: entry.uv,
                    params: [0.0, 0.0, 0.0, 0.0],
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

fn hud_layout(atlas: &GlyphAtlas, hud: &CompanionHudText, stack_size: f32) -> HudLayout {
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

fn hud_line_layout(atlas: &GlyphAtlas, text: &str, font_size: f32) -> HudLineLayout {
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

fn collect_glyphs(plan: &SmoothCompanionScenePlan, hud: &CompanionHudText) -> Vec<(String, bool)> {
    let mut glyphs = BTreeSet::new();
    for layer in &plan.layers {
        for item in &layer.items {
            if let SmoothLayerItem::LocalCell(cell) = item {
                if let Some(glyph) = cell.glyph.as_ref() {
                    glyphs.insert((glyph.clone(), cell.bold));
                }
            }
        }
    }
    for glyph in hud
        .today_total
        .chars()
        .chain(hud.daily_percent.chars())
        .chain(hud.pace.chars())
    {
        glyphs.insert((glyph.to_string(), false));
    }
    glyphs.into_iter().collect()
}

fn build_glyph_atlas(
    glyphs: &[(String, bool)],
) -> std::result::Result<GlyphAtlas, RetainedFailureCategory> {
    let count = glyphs.len().max(1) as u32;
    let rows = count.div_ceil(ATLAS_COLUMNS).min(MAX_ATLAS_ROWS);
    if count > ATLAS_COLUMNS * MAX_ATLAS_ROWS {
        return Err(RetainedFailureCategory::AtlasUnavailable);
    }
    let width = ATLAS_COLUMNS * ATLAS_CELL;
    let height = rows * ATLAS_CELL;
    let mut rgba = vec![0_u8; (width * height * 4) as usize];
    let mut entries = BTreeMap::new();
    for (index, (glyph, bold)) in glyphs.iter().enumerate() {
        let slot_x = index as u32 % ATLAS_COLUMNS;
        let slot_y = index as u32 / ATLAS_COLUMNS;
        let entry = rasterize_glyph(
            glyph,
            *bold,
            &mut rgba,
            width,
            height,
            slot_x * ATLAS_CELL,
            slot_y * ATLAS_CELL,
        )?;
        entries.insert((glyph.clone(), *bold), entry);
    }
    Ok(GlyphAtlas { width, height, rgba, entries })
}

fn rasterize_glyph(
    glyph: &str,
    bold: bool,
    atlas: &mut [u8],
    atlas_width: u32,
    atlas_height: u32,
    x: u32,
    y: u32,
) -> std::result::Result<GlyphAtlasEntry, RetainedFailureCategory> {
    unsafe {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(), std::ptr::null_mut(), ATLAS_CELL as isize, ATLAS_CELL as isize,
            8, 4, true, false, NSDeviceRGBColorSpace, (ATLAS_CELL * 4) as isize, 32,
        ).ok_or(RetainedFailureCategory::AtlasUnavailable)?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)
            .ok_or(RetainedFailureCategory::AtlasUnavailable)?;
        let previous = NSGraphicsContext::currentContext();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let text = NSString::from_str(glyph);
        let font = NSFont::monospacedSystemFontOfSize_weight(
            GLYPH_FONT_SIZE,
            if bold { NSFontWeightBold } else { 0.0 },
        );
        let mut attributed = NSMutableAttributedString::from_nsstring(&text);
        let range = NSRange::from(0..text.length());
        attributed.addAttribute_value_range(NSFontAttributeName, &font, range);
        let white = NSColor::whiteColor();
        attributed.addAttribute_value_range(NSForegroundColorAttributeName, &white, range);
        let attributed: Retained<objc2_foundation::NSAttributedString> =
            Retained::into_super(attributed);
        let size = attributed.size();
        if size.width + f64::from(ATLAS_PADDING * 2) > f64::from(ATLAS_CELL)
            || size.height + f64::from(ATLAS_PADDING * 2) > f64::from(ATLAS_CELL)
        {
            NSGraphicsContext::setCurrentContext(previous.as_deref());
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        let draw_x = f64::from(ATLAS_PADDING);
        let draw_y = f64::from(ATLAS_PADDING);
        attributed.drawAtPoint(NSPoint::new(draw_x, draw_y));
        context.flushGraphics();
        NSGraphicsContext::setCurrentContext(previous.as_deref());
        let data = rep.bitmapData();
        if data.is_null() {
            return Err(RetainedFailureCategory::AtlasUnavailable);
        }
        let mut ink_min_x = ATLAS_CELL;
        let mut ink_min_y = ATLAS_CELL;
        let mut ink_max_x = 0;
        let mut ink_max_y = 0;
        let mut has_ink = false;
        for row in 0..ATLAS_CELL {
            for col in 0..ATLAS_CELL {
                let src = ((row * ATLAS_CELL + col) * 4) as usize;
                let dst = (((y + row) * atlas_width + x + col) * 4) as usize;
                let alpha = *data.add(src + 3);
                atlas[dst..dst + 4].copy_from_slice(&[255, 255, 255, alpha]);
                if alpha != 0 {
                    has_ink = true;
                    ink_min_x = ink_min_x.min(col);
                    ink_min_y = ink_min_y.min(row);
                    ink_max_x = ink_max_x.max(col + 1);
                    ink_max_y = ink_max_y.max(row + 1);
                }
            }
        }
        let (uv, ink_origin, ink_size) = if has_ink {
            (
                [
                    (x + ink_min_x) as f32 / atlas_width as f32,
                    (y + ink_min_y) as f32 / atlas_height as f32,
                    (x + ink_max_x) as f32 / atlas_width as f32,
                    (y + ink_max_y) as f32 / atlas_height as f32,
                ],
                [
                    ink_min_x as f32 - draw_x as f32,
                    ink_min_y as f32 - draw_y as f32,
                ],
                [
                    (ink_max_x - ink_min_x) as f32,
                    (ink_max_y - ink_min_y) as f32,
                ],
            )
        } else {
            ([0.0; 4], [0.0; 2], [0.0; 2])
        };
        Ok(GlyphAtlasEntry {
            uv,
            ink_origin,
            ink_size,
            advance: size.width as f32,
            line_height: size.height as f32,
        })
    }
}

fn glyph_scale(font_size: f32) -> f32 {
    font_size / GLYPH_FONT_SIZE as f32
}

fn glyph_atlas_needs_rebuild(
    cached_glyphs: Option<&Vec<(String, bool)>>,
    requested_glyphs: &[(String, bool)],
) -> bool {
    cached_glyphs.is_none_or(|cached| cached.as_slice() != requested_glyphs)
}

fn glyph_advance(entry: GlyphAtlasEntry, font_size: f32) -> f32 {
    entry.advance * glyph_scale(font_size)
}

fn glyph_run_width(atlas: &GlyphAtlas, text: &str, font_size: f32) -> f32 {
    text.chars()
        .filter_map(|glyph| atlas.entries.get(&(glyph.to_string(), false)).copied())
        .map(|entry| glyph_advance(entry, font_size))
        .sum()
}

fn glyph_run_height(atlas: &GlyphAtlas, text: &str, font_size: f32) -> f32 {
    text.chars()
        .filter_map(|glyph| atlas.entries.get(&(glyph.to_string(), false)).copied())
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
    use super::{
        glyph_advance, glyph_atlas_needs_rebuild, glyph_ink_rect, hud_layout, physical_dimension,
        push_analytic_arc, srgb_channel_to_linear, GlyphAtlas, GlyphAtlasEntry,
    };
    use std::collections::BTreeMap;

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
        let entry = GlyphAtlasEntry {
            uv: [0.1, 0.2, 0.3, 0.4],
            ink_origin: [3.0, 7.0],
            ink_size: [20.0, 31.0],
            advance: 29.0,
            line_height: 52.0,
        };

        assert_eq!(
            glyph_ink_rect([100.0, 200.0], 24.0, entry),
            Some([101.5, 203.5, 10.0, 15.5])
        );
        assert_eq!(glyph_advance(entry, 24.0), 14.5);
    }

    #[test]
    fn blank_glyph_has_advance_without_a_visible_quad() {
        let entry = GlyphAtlasEntry {
            uv: [0.0; 4],
            ink_origin: [0.0; 2],
            ink_size: [0.0; 2],
            advance: 28.0,
            line_height: 52.0,
        };

        assert_eq!(glyph_ink_rect([12.0, 34.0], 48.0, entry), None);
        assert_eq!(glyph_advance(entry, 48.0), 28.0);
    }

    #[test]
    fn glyph_atlas_rebuilds_only_when_the_repertoire_changes() {
        let cached = vec![("A".to_string(), false), ("B".to_string(), true)];
        assert!(!glyph_atlas_needs_rebuild(Some(&cached), &cached));
        assert!(glyph_atlas_needs_rebuild(None, &cached));
        assert!(glyph_atlas_needs_rebuild(
            Some(&cached),
            &[("A".to_string(), false)]
        ));
    }

    #[test]
    fn hud_layout_uses_the_same_big_and_subline_font_policy_as_smooth() {
        let entry = GlyphAtlasEntry {
            uv: [0.0; 4],
            ink_origin: [0.0; 2],
            ink_size: [10.0, 20.0],
            advance: 30.0,
            line_height: 50.0,
        };
        let atlas = GlyphAtlas {
            width: 1,
            height: 1,
            rgba: vec![0; 4],
            entries: [('A'.to_string(), false), ('B'.to_string(), false)]
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
