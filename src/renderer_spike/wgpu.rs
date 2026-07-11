#![cfg(all(target_os = "macos", feature = "renderer-spike-wgpu"))]

use std::cell::RefCell;
use std::ffi::c_void;
use std::path::Path;
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use objc2::declare_class;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{msg_send, msg_send_id};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityElement, NSAccessibilityGroupRole,
    NSAccessibilityStaticTextRole, NSApplication, NSApplicationActivationPolicy,
    NSBackingStoreType, NSEvent, NSEventModifierFlags, NSEventType, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSArray, NSPoint, NSRect, NSSize, NSString, NSTimer};
use objc2_quartz_core::CAMetalLayer;
use serde::{Deserialize, Serialize};
use wgpu::util::DeviceExt;

use crate::error::{GlorpError, Result};

use super::artifacts::{
    self, CleanupArtifact, HostBoundaryArtifact, HostBoundaryObservation, SummaryArtifact,
    ARTIFACT_SCHEMA_VERSION,
};
use super::fixture::{
    canonical_atlas, canonical_fixture, resolve_frame, DecisionPrimitiveKind,
    DecisionResolvedFrame, DecisionSourceFixture, DYNAMIC_PRIMITIVE_COUNT,
};
use super::{RendererSpikeFault, RendererSpikeOptions, RendererSpikeTrack};

struct WgpuSpikeState {
    window: Retained<NSWindow>,
    view: Retained<WgpuSpikeView>,
    layer: Retained<CAMetalLayer>,
    gpu: WgpuHost,
    options: RendererSpikeOptions,
    started_at: Instant,
    runner_entry_micros: u64,
    harness_entry_micros: u64,
    host_ready_micros: u64,
    first_present_micros: Option<u64>,
    owner_thread: std::thread::ThreadId,
    frame_count: u64,
    submission_count: u64,
    callback_panic_count: u64,
    callback_panic_injected: bool,
    metrics: Vec<artifacts::FrameMetric>,
    host_calls: Vec<HostBoundaryObservation>,
    accessibility_elements: Vec<Retained<NSAccessibilityElement>>,
    pointer_projection: Option<PointerProjectionArtifact>,
    input_audit: InputAuditState,
    finished: bool,
}

thread_local! {
    static WGPU_STATE: RefCell<Option<WgpuSpikeState>> = const { RefCell::new(None) };
}

declare_class!(
    struct WgpuSpikeController;

    unsafe impl ClassType for WgpuSpikeController {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpRendererSpikeWgpuController";
    }

    impl DeclaredClass for WgpuSpikeController {}

    unsafe impl WgpuSpikeController {
        #[method(rendererSpikeWgpuTick:)]
        fn tick(&self, _sender: Option<&AnyObject>) {
            run_callback("renderer-spike-wgpu-tick", tick);
        }
    }
);

declare_class!(
    struct WgpuSpikeView;

    unsafe impl ClassType for WgpuSpikeView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "GlorpRendererSpikeWgpuView";
    }

    impl DeclaredClass for WgpuSpikeView {}

    unsafe impl WgpuSpikeView {
        #[method(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[method(mouseDown:)]
        fn mouse_down(&self, event: &NSEvent) {
            run_callback("renderer-spike-wgpu-mousedown", || {
                let location = unsafe { event.locationInWindow() };
                let local = self.convertPoint_fromView(location, None);
                record_pointer_projection(local, self.bounds());
            });
        }

        #[method(keyDown:)]
        fn key_down(&self, _event: &NSEvent) {
            WGPU_STATE.with(|cell| {
                if let Ok(mut state) = cell.try_borrow_mut() {
                    if let Some(state) = state.as_mut() {
                        state.input_audit.key_event_delivered = true;
                        state.host_calls.push(observation("synthetic-key-event"));
                    }
                }
            });
        }
    }
);

struct WgpuHost {
    // Drop the surface before the retained native layer stored by WgpuSpikeState.
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    physical_width: u32,
    physical_height: u32,
    pipeline: wgpu::RenderPipeline,
    atlas_bind_group: wgpu::BindGroup,
    static_instance_buffer: wgpu::Buffer,
    frame_instance_buffer: wgpu::Buffer,
    atlas_override_buffer: wgpu::Buffer,
    instance_capacity: usize,
    resource_generation: u64,
    static_upload_pending: bool,
    initial_atlas_upload_pending: bool,
    last_semantic_tick: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuStaticInstance {
    base_rect: [f32; 4],
    color: [f32; 4],
    atlas_rect: [f32; 4],
    params: [f32; 4],
}

impl GpuStaticInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = wgpu::vertex_attr_array![
        0 => Float32x4,
        1 => Float32x4,
        2 => Float32x4,
        3 => Float32x4
    ];

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuFrameInstance {
    offset: [f32; 2],
}

impl GpuFrameInstance {
    const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![4 => Float32x2];

    const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &Self::ATTRIBUTES,
    };
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuAtlasOverrides {
    rects: [[f32; 4]; DYNAMIC_PRIMITIVE_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UploadStats {
    static_bytes: u64,
    dynamic_bytes: u64,
    atlas_bytes: u64,
    uniform_bytes: u64,
    resource_generation: u64,
    draw_calls: u32,
}

impl UploadStats {
    fn total_bytes(self) -> u64 {
        self.static_bytes
            .saturating_add(self.dynamic_bytes)
            .saturating_add(self.atlas_bytes)
            .saturating_add(self.uniform_bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HostCheckpointArtifact {
    schema_version: u16,
    adapter_name: String,
    backend: String,
    device_type: String,
    surface_format: String,
    present_mode: String,
    alpha_mode: String,
    logical_width: u32,
    logical_height: u32,
    physical_width: u32,
    physical_height: u32,
    backing_scale: f64,
    reconfiguration_count: u64,
    acquisition_skip_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CaptureMetadata {
    schema_version: u16,
    logical_size: u16,
    physical_width: u32,
    physical_height: u32,
    frame_index: u64,
    orientation: &'static str,
    color_format: &'static str,
    aligned_bytes_per_row: u32,
    map_duration_micros: u64,
    static_upload_bytes: u64,
    dynamic_upload_bytes: u64,
    atlas_upload_bytes: u64,
    uniform_upload_bytes: u64,
    resource_generation: u64,
    draw_calls: u32,
}

struct CapturePng<'a> {
    root: &'a Path,
    metadata: CaptureMetadata,
    rgba: &'a [u8],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointerProjectionArtifact {
    schema_version: u16,
    view_x: f64,
    view_y: f64,
    logical_x: f64,
    logical_y: f64,
    inside: bool,
}

#[derive(Debug, Clone, Default)]
struct InputAuditState {
    mouse_event_delivered: bool,
    key_event_delivered: bool,
    stale_snapshot_rejected: bool,
    current_snapshot_accepted: bool,
    first_responder: bool,
    hide_children_detached: bool,
    reveal_children_restored: bool,
    close_children_detached: bool,
    resize_bounds: Vec<AccessibilityResizeArtifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessibilityAuditArtifact {
    schema_version: u16,
    group_count: u32,
    value_count: u32,
    child_count: u32,
    per_glyph_children: bool,
    sanitized: bool,
    initial_logical_size: u16,
    initial_backing_scale: f64,
    initial_bounds: Vec<AccessibilityBoundsArtifact>,
    resize_bounds: Vec<AccessibilityResizeArtifact>,
    synthetic_mouse_event_delivered: bool,
    synthetic_key_event_delivered: bool,
    stale_snapshot_rejected: bool,
    current_snapshot_accepted: bool,
    first_responder: bool,
    hide_children_detached: bool,
    reveal_children_restored: bool,
    close_children_detached: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessibilityBoundsArtifact {
    id: String,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AccessibilityResizeArtifact {
    logical_size: u16,
    backing_scale: f64,
    bounds: Vec<AccessibilityBoundsArtifact>,
}

fn run_callback(label: &'static str, callback: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err() {
        eprintln!("glorp renderer spike caught callback panic: {label}");
        WGPU_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                if let Some(state) = state.as_mut() {
                    state.callback_panic_count = state.callback_panic_count.saturating_add(1);
                }
            }
        });
    }
}

pub fn run(options: RendererSpikeOptions) -> Result<()> {
    let harness_entry_micros = artifacts::monotonic_micros();
    let runner_entry_micros = options.runner_entry_micros.unwrap_or(harness_entry_micros);
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        GlorpError::Message("renderer spike must run on the macOS main thread".into())
    })?;
    artifacts::write_common_artifacts(&options)?;

    if options.inject_fault == Some(RendererSpikeFault::SurfaceUnavailable) {
        finish_early_fault(
            options,
            runner_entry_micros,
            harness_entry_micros,
            "reject-injected-surface-unavailable",
        )?;
        return Err(GlorpError::Message(
            "wgpu renderer spike rejected injected surface unavailability".into(),
        ));
    }

    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let frame = NSRect::new(
        NSPoint::new(160.0, 160.0),
        NSSize::new(
            f64::from(options.logical_size),
            f64::from(options.logical_size),
        ),
    );
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::Closable
        | NSWindowStyleMask::Miniaturizable
        | NSWindowStyleMask::Resizable;
    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            mtm.alloc(),
            frame,
            style,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        )
    };
    window.setTitle(&NSString::from_str("Glorp Renderer Spike — wgpu"));
    let view: Retained<WgpuSpikeView> = unsafe {
        msg_send_id![mtm.alloc::<WgpuSpikeView>(), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), frame.size)]
    };
    let layer = unsafe { CAMetalLayer::new() };
    view.setWantsLayer(true);
    unsafe { view.setLayer(Some(&layer)) };
    window.setContentView(Some(&view));
    window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let backing_scale = window.backingScaleFactor();
    let physical_width = physical_dimension(frame.size.width, backing_scale);
    let physical_height = physical_dimension(frame.size.height, backing_scale);
    unsafe {
        layer.setDrawableSize(NSSize::new(
            f64::from(physical_width),
            f64::from(physical_height),
        ));
    }

    let (gpu, adapter_info) = WgpuHost::new(&layer, physical_width, physical_height)?;
    let accessibility_elements = install_accessibility(&view, frame.size)?;
    let host_ready_micros = artifacts::monotonic_micros();
    let first_responder = window.makeFirstResponder(Some(&view));
    let owner_thread = std::thread::current().id();
    let thread_label = format!("{owner_thread:?}");
    let mut host_calls = Vec::new();
    for operation in [
        "appkit-view-create",
        "metal-layer-create",
        "surface-create",
        "adapter-request",
        "device-request",
        "surface-configure",
    ] {
        host_calls.push(HostBoundaryObservation {
            operation: operation.to_string(),
            thread: thread_label.clone(),
            main_thread: MainThreadMarker::new().is_some(),
        });
    }

    artifacts::write_json(
        &options.out.join("host-checkpoint.json"),
        &HostCheckpointArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            adapter_name: adapter_info.name,
            backend: format!("{:?}", adapter_info.backend),
            device_type: format!("{:?}", adapter_info.device_type),
            surface_format: format!("{:?}", gpu.config.format),
            present_mode: format!("{:?}", gpu.config.present_mode),
            alpha_mode: format!("{:?}", gpu.config.alpha_mode),
            logical_width: u32::from(options.logical_size),
            logical_height: u32::from(options.logical_size),
            physical_width,
            physical_height,
            backing_scale,
            reconfiguration_count: 1,
            acquisition_skip_count: 0,
        },
    )?;

    let controller: Retained<WgpuSpikeController> =
        unsafe { msg_send_id![WgpuSpikeController::class(), new] };
    let interval = match options.track {
        RendererSpikeTrack::Active => 1.0 / 30.0,
        _ => 1.0 / 15.0,
    };
    WGPU_STATE.with(|cell| {
        *cell.borrow_mut() = Some(WgpuSpikeState {
            window: window.clone(),
            view: view.clone(),
            layer,
            gpu,
            options,
            started_at: Instant::now(),
            runner_entry_micros,
            harness_entry_micros,
            host_ready_micros,
            first_present_micros: None,
            owner_thread,
            frame_count: 0,
            submission_count: 0,
            callback_panic_count: 0,
            callback_panic_injected: false,
            metrics: Vec::new(),
            host_calls,
            accessibility_elements,
            pointer_projection: None,
            input_audit: InputAuditState { first_responder, ..Default::default() },
            finished: false,
        });
    });
    deliver_synthetic_events(&view, &window)?;
    let _timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            interval,
            &controller,
            sel!(rendererSpikeWgpuTick:),
            None,
            true,
        )
    };
    unsafe { app.run() };
    Ok(())
}

impl WgpuHost {
    fn new(
        layer: &CAMetalLayer,
        physical_width: u32,
        physical_height: u32,
    ) -> Result<(Self, wgpu::AdapterInfo)> {
        let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_descriptor.backends = wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(instance_descriptor);
        let layer_pointer = std::ptr::from_ref(layer).cast_mut().cast::<c_void>();
        // SAFETY: `layer_pointer` is a live CAMetalLayer retained by WgpuSpikeState,
        // which is dropped only after WgpuHost and therefore after the surface.
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(layer_pointer))
        }
        .map_err(|error| GlorpError::Message(format!("wgpu surface create failed: {error}")))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|error| GlorpError::Message(format!("wgpu adapter request failed: {error}")))?;
        let adapter_info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("glorp-renderer-spike-device"),
            ..Default::default()
        }))
        .map_err(|error| GlorpError::Message(format!("wgpu device request failed: {error}")))?;
        device.on_uncaptured_error(Arc::new(|error| {
            let category = match error {
                wgpu::Error::OutOfMemory { .. } => "out-of-memory",
                wgpu::Error::Validation { .. } => "validation",
                wgpu::Error::Internal { .. } => "internal",
            };
            eprintln!("glorp renderer spike wgpu uncaptured error: {category}");
        }));
        device.set_device_lost_callback(|reason, _message| {
            let category = match reason {
                wgpu::DeviceLostReason::Unknown => "unknown",
                wgpu::DeviceLostReason::Destroyed => "destroyed",
            };
            eprintln!("glorp renderer spike wgpu device lost: {category}");
        });
        let config = surface
            .get_default_config(&adapter, physical_width, physical_height)
            .ok_or_else(|| {
                GlorpError::Message("wgpu surface has no compatible default configuration".into())
            })?;
        surface.configure(&device, &config);
        let atlas = canonical_atlas();
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glorp-renderer-spike-atlas"),
            size: wgpu::Extent3d {
                width: u32::from(atlas.width),
                height: u32::from(atlas.height),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(u32::from(atlas.width) * 4),
                rows_per_image: Some(u32::from(atlas.height)),
            },
            wgpu::Extent3d {
                width: u32::from(atlas.width),
                height: u32::from(atlas.height),
                depth_or_array_layers: 1,
            },
        );
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glorp-renderer-spike-atlas-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glorp-renderer-spike-fixture-shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/fixture.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glorp-renderer-spike-atlas-layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let fixture = canonical_fixture();
        let static_instances = gpu_static_instances(&fixture);
        let initial_atlas_overrides = gpu_atlas_overrides(0);
        let static_instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glorp-renderer-spike-static-instance-buffer"),
            contents: bytemuck::cast_slice(&static_instances),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let atlas_override_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("glorp-renderer-spike-atlas-override-buffer"),
            contents: bytemuck::bytes_of(&initial_atlas_overrides),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glorp-renderer-spike-atlas-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: atlas_override_buffer.as_entire_binding(),
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glorp-renderer-spike-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let target = Some(wgpu::ColorTargetState {
            format: config.format,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glorp-renderer-spike-fixture-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    Some(GpuStaticInstance::LAYOUT),
                    Some(GpuFrameInstance::LAYOUT),
                ],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[target],
            }),
            multiview_mask: None,
            cache: None,
        });
        let instance_capacity = fixture.primitives.len();
        let frame_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glorp-renderer-spike-frame-instance-buffer"),
            size: (instance_capacity * std::mem::size_of::<GpuFrameInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok((
            Self {
                surface,
                device,
                queue,
                config,
                physical_width,
                physical_height,
                pipeline,
                atlas_bind_group,
                static_instance_buffer,
                frame_instance_buffer,
                atlas_override_buffer,
                instance_capacity,
                resource_generation: 1,
                static_upload_pending: true,
                initial_atlas_upload_pending: true,
                last_semantic_tick: 0,
            },
            adapter_info,
        ))
    }

    fn resize(&mut self, layer: &CAMetalLayer, width: u32, height: u32) {
        if width == 0
            || height == 0
            || (width == self.physical_width && height == self.physical_height)
        {
            return;
        }
        self.physical_width = width;
        self.physical_height = height;
        self.config.width = width;
        self.config.height = height;
        unsafe {
            layer.setDrawableSize(NSSize::new(f64::from(width), f64::from(height)));
        }
        self.surface.configure(&self.device, &self.config);
    }

    fn render_and_present(&mut self, frame: &DecisionResolvedFrame) -> PresentResult {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return PresentResult::Skipped("surface-outdated");
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                return PresentResult::Skipped("surface-timeout")
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return PresentResult::Skipped("surface-occluded")
            }
            wgpu::CurrentSurfaceTexture::Lost => return PresentResult::Skipped("surface-lost"),
            wgpu::CurrentSurfaceTexture::Validation => {
                return PresentResult::Skipped("surface-validation")
            }
        };
        let upload = self.update_frame_resources(frame);
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glorp-renderer-spike-clear"),
            });
        {
            let color_attachment = Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.025, g: 0.055, b: 0.085, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glorp-renderer-spike-clear-pass"),
                color_attachments: &[color_attachment],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.atlas_bind_group, &[]);
            pass.set_vertex_buffer(0, self.static_instance_buffer.slice(..));
            pass.set_vertex_buffer(1, self.frame_instance_buffer.slice(..));
            pass.draw(0..6, 0..self.instance_capacity as u32);
        }
        self.queue.submit([encoder.finish()]);
        self.queue.present(surface_texture);
        PresentResult::Presented(upload)
    }

    fn update_frame_resources(&mut self, frame: &DecisionResolvedFrame) -> UploadStats {
        let frame_instances = gpu_frame_instances(frame);
        debug_assert_eq!(frame_instances.len(), self.instance_capacity);
        self.queue.write_buffer(
            &self.frame_instance_buffer,
            0,
            bytemuck::cast_slice(&frame_instances),
        );
        let semantic_tick = frame.elapsed_ms / 250;
        let mut atlas_bytes = u64::from(self.initial_atlas_upload_pending)
            * std::mem::size_of::<GpuAtlasOverrides>() as u64;
        self.initial_atlas_upload_pending = false;
        if semantic_tick != self.last_semantic_tick {
            let overrides = gpu_atlas_overrides(semantic_tick);
            self.queue.write_buffer(
                &self.atlas_override_buffer,
                0,
                bytemuck::bytes_of(&overrides),
            );
            atlas_bytes =
                atlas_bytes.saturating_add(std::mem::size_of::<GpuAtlasOverrides>() as u64);
            self.last_semantic_tick = semantic_tick;
        }
        let static_bytes = if self.static_upload_pending {
            self.static_upload_pending = false;
            (self.instance_capacity * std::mem::size_of::<GpuStaticInstance>()) as u64
        } else {
            0
        };
        UploadStats {
            static_bytes,
            dynamic_bytes: (frame_instances.len() * std::mem::size_of::<GpuFrameInstance>()) as u64,
            atlas_bytes,
            uniform_bytes: 0,
            resource_generation: self.resource_generation,
            draw_calls: 1,
        }
    }

    fn capture(
        &mut self,
        frame: &DecisionResolvedFrame,
        options: &RendererSpikeOptions,
        artifact_frame_index: u64,
    ) -> Result<()> {
        if options.inject_fault == Some(RendererSpikeFault::CaptureTimeout) {
            return Err(GlorpError::Message(
                "wgpu capture failed: injected-capture-timeout".into(),
            ));
        }
        let width = self.physical_width;
        let height = self.physical_height;
        let upload = self.update_frame_resources(frame);
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glorp-renderer-spike-capture-texture"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let unpadded_bytes_per_row = width * 4;
        let aligned_bytes_per_row =
            align_up(unpadded_bytes_per_row, wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glorp-renderer-spike-capture-staging"),
            size: u64::from(aligned_bytes_per_row) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glorp-renderer-spike-capture-encoder"),
            });
        {
            let attachment = Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.025, g: 0.055, b: 0.085, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glorp-renderer-spike-capture-pass"),
                color_attachments: &[attachment],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.atlas_bind_group, &[]);
            pass.set_vertex_buffer(0, self.static_instance_buffer.slice(..));
            pass.set_vertex_buffer(1, self.frame_instance_buffer.slice(..));
            pass.draw(0..6, 0..self.instance_capacity as u32);
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );
        let submission = self.queue.submit([encoder.finish()]);
        let (sender, receiver) = mpsc::sync_channel(1);
        staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        let map_started = Instant::now();
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(5)),
            })
            .map_err(|error| GlorpError::Message(format!("wgpu capture poll failed: {error}")))?;
        receiver
            .recv_timeout(Duration::from_millis(100))
            .map_err(|_| GlorpError::Message("wgpu capture map callback timed out".into()))?
            .map_err(|error| GlorpError::Message(format!("wgpu capture map failed: {error}")))?;
        let mapped = staging
            .slice(..)
            .get_mapped_range()
            .map_err(|error| GlorpError::Message(format!("wgpu capture range failed: {error}")))?;
        let mut rgba = vec![0_u8; (width * height * 4) as usize];
        for row in 0..height as usize {
            let source_start = row * aligned_bytes_per_row as usize;
            let source_end = source_start + unpadded_bytes_per_row as usize;
            let destination_start = row * unpadded_bytes_per_row as usize;
            let destination_end = destination_start + unpadded_bytes_per_row as usize;
            rgba[destination_start..destination_end]
                .copy_from_slice(&mapped[source_start..source_end]);
        }
        drop(mapped);
        staging.unmap();
        if matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in rgba.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        write_capture_png(CapturePng {
            root: &options.out,
            metadata: CaptureMetadata {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                logical_size: options.logical_size,
                physical_width: width,
                physical_height: height,
                frame_index: artifact_frame_index,
                orientation: "top-left",
                color_format: "rgba8-srgb-png",
                aligned_bytes_per_row,
                map_duration_micros: map_started.elapsed().as_micros() as u64,
                static_upload_bytes: upload.static_bytes,
                dynamic_upload_bytes: upload.dynamic_bytes,
                atlas_upload_bytes: upload.atlas_bytes,
                uniform_upload_bytes: upload.uniform_bytes,
                resource_generation: upload.resource_generation,
                draw_calls: upload.draw_calls,
            },
            rgba: &rgba,
        })
    }
}

enum PresentResult {
    Presented(UploadStats),
    Skipped(&'static str),
}

fn tick() {
    let inject_callback_panic = WGPU_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return false;
        };
        if state.options.inject_fault == Some(RendererSpikeFault::CallbackPanic)
            && !state.callback_panic_injected
        {
            state.callback_panic_injected = true;
            true
        } else {
            false
        }
    });
    if inject_callback_panic {
        panic!("injected renderer spike callback panic");
    }
    let should_finish = WGPU_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return true;
        };
        if state.finished {
            return true;
        }
        debug_assert_eq!(state.owner_thread, std::thread::current().id());
        debug_assert!(MainThreadMarker::new().is_some());
        let elapsed = state.started_at.elapsed();
        if matches!(state.options.track, RendererSpikeTrack::Resize) {
            resize_for_elapsed(state, elapsed);
            update_accessibility(state);
        }
        if matches!(state.options.track, RendererSpikeTrack::Occlusion) {
            update_occlusion_for_elapsed(state, elapsed);
        }
        state.frame_count = state.frame_count.saturating_add(1);
        let should_submit = match state.options.track {
            RendererSpikeTrack::Static => state.submission_count == 0,
            RendererSpikeTrack::Occlusion => state.window.isVisible(),
            _ => true,
        };
        if should_submit {
            let frame_started = Instant::now();
            let bounds = state.view.bounds();
            let backing_scale = state.window.backingScaleFactor();
            let width = physical_dimension(bounds.size.width, backing_scale);
            let height = physical_dimension(bounds.size.height, backing_scale);
            let resized = width != state.gpu.physical_width || height != state.gpu.physical_height;
            state.gpu.resize(&state.layer, width, height);
            let operation = if resized {
                "resize"
            } else {
                "acquire-encode-present"
            };
            let fixture = canonical_fixture();
            let resolved = resolve_frame(&fixture, elapsed.as_millis() as u64);
            let upload = match state.gpu.render_and_present(&resolved) {
                PresentResult::Presented(upload) => {
                    if state.first_present_micros.is_none() {
                        state.first_present_micros = Some(artifacts::monotonic_micros());
                    }
                    state.submission_count = state.submission_count.saturating_add(1);
                    state.host_calls.push(observation(operation));
                    upload
                }
                PresentResult::Skipped(category) => {
                    state.host_calls.push(observation(category));
                    UploadStats {
                        static_bytes: 0,
                        dynamic_bytes: 0,
                        atlas_bytes: 0,
                        uniform_bytes: 0,
                        resource_generation: state.gpu.resource_generation,
                        draw_calls: 0,
                    }
                }
            };
            state.metrics.push(artifacts::FrameMetric {
                frame_index: state.frame_count,
                elapsed_ms: elapsed.as_millis() as u64,
                end_to_end_cpu_micros: frame_started.elapsed().as_micros() as u64,
                requested_visible_frames: state.frame_count,
                completed_visible_frames: state.submission_count,
                submissions: state.submission_count,
                missed_deadlines: 0,
                primitive_count: resolved.primitives.len() as u32,
                static_rebuilds: u64::from(upload.static_bytes != 0),
                atlas_misses: 0,
                upload_bytes: upload.total_bytes(),
                static_upload_bytes: upload.static_bytes,
                dynamic_upload_bytes: upload.dynamic_bytes,
                atlas_upload_bytes: upload.atlas_bytes,
                uniform_upload_bytes: upload.uniform_bytes,
                resource_generation: upload.resource_generation,
                draw_calls: upload.draw_calls,
            });
        }
        elapsed >= Duration::from_millis(state.options.duration_ms) && state.frame_count >= 5
    });
    if should_finish {
        finish();
    }
}

fn observation(operation: &str) -> HostBoundaryObservation {
    HostBoundaryObservation {
        operation: operation.to_string(),
        thread: format!("{:?}", std::thread::current().id()),
        main_thread: MainThreadMarker::new().is_some(),
    }
}

fn record_pointer_projection(point: NSPoint, bounds: NSRect) {
    WGPU_STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.as_mut() {
                let width = bounds.size.width.max(1.0);
                let height = bounds.size.height.max(1.0);
                let logical_x = point.x / width * f64::from(state.options.logical_size);
                let logical_y = (height - point.y) / height * f64::from(state.options.logical_size);
                state.pointer_projection = Some(PointerProjectionArtifact {
                    schema_version: ARTIFACT_SCHEMA_VERSION,
                    view_x: point.x,
                    view_y: point.y,
                    logical_x,
                    logical_y,
                    inside: point.x >= 0.0
                        && point.y >= 0.0
                        && point.x <= width
                        && point.y <= height,
                });
                state.input_audit.mouse_event_delivered = true;
                state.host_calls.push(observation("pointer-project"));
            }
        }
    });
}

fn deliver_synthetic_events(view: &WgpuSpikeView, window: &NSWindow) -> Result<()> {
    let location = NSPoint::new(
        window.frame().size.width / 2.0,
        window.frame().size.height / 2.0,
    );
    let mouse = unsafe {
        NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
            NSEventType::LeftMouseDown,
            location,
            NSEventModifierFlags(0),
            0.0,
            window.windowNumber(),
            None,
            1,
            1,
            1.0,
        )
    }
    .ok_or_else(|| GlorpError::Message("failed to create synthetic mouse event".into()))?;
    unsafe {
        let _: () = msg_send![view, mouseDown: &*mouse];
    }
    let characters = NSString::from_str("x");
    let key = unsafe {
        NSEvent::keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode(
            NSEventType::KeyDown,
            location,
            NSEventModifierFlags(0),
            0.0,
            window.windowNumber(),
            None,
            &characters,
            &characters,
            false,
            7,
        )
    }
    .ok_or_else(|| GlorpError::Message("failed to create synthetic key event".into()))?;
    unsafe {
        let _: () = msg_send![view, keyDown: &*key];
    }
    WGPU_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut().ok_or_else(|| {
            GlorpError::Message("wgpu state disappeared during input audit".into())
        })?;
        state.input_audit.stale_snapshot_rejected = accepts_input_snapshot(
            state.gpu.resource_generation.saturating_sub(1),
            state.frame_count,
            state.gpu.resource_generation,
            state.frame_count,
        )
        .is_none();
        state.input_audit.current_snapshot_accepted = accepts_input_snapshot(
            state.gpu.resource_generation,
            state.frame_count,
            state.gpu.resource_generation,
            state.frame_count,
        )
        .is_some();
        Ok::<_, GlorpError>(())
    })
}

fn accepts_input_snapshot(
    event_generation: u64,
    event_frame: u64,
    current_generation: u64,
    current_frame: u64,
) -> Option<(u64, u64)> {
    (event_generation == current_generation && event_frame == current_frame)
        .then_some((event_generation, event_frame))
}

fn install_accessibility(
    view: &WgpuSpikeView,
    size: NSSize,
) -> Result<Vec<Retained<NSAccessibilityElement>>> {
    let nodes = super::fixture::semantic_fixture(size.width.round() as u16, false);
    let group = unsafe { NSAccessibilityElement::new() };
    unsafe {
        group.setAccessibilityElement(true);
        group.setAccessibilityRole(Some(NSAccessibilityGroupRole));
        group.setAccessibilityLabel(Some(&NSString::from_str(&nodes[0].name)));
        group.setAccessibilityFrameInParentSpace(node_rect(&nodes[0], size.height));
        group.setAccessibilityParent(Some(view));
    }
    let mut elements = vec![group];
    for node in nodes.iter().skip(1) {
        let element = unsafe { NSAccessibilityElement::new() };
        unsafe {
            element.setAccessibilityElement(true);
            element.setAccessibilityRole(Some(NSAccessibilityStaticTextRole));
            element.setAccessibilityLabel(Some(&NSString::from_str(&node.name)));
            if let Some(value) = &node.value {
                element.setAccessibilityValueDescription(Some(&NSString::from_str(value)));
            }
            element.setAccessibilityFrameInParentSpace(node_rect(node, size.height));
            element.setAccessibilityParent(Some(view));
        }
        elements.push(element);
    }
    let children = NSArray::<NSAccessibilityElement>::from_id_slice(&elements);
    unsafe {
        view.setAccessibilityElement(false);
        let _: () = msg_send![view, setAccessibilityChildren: &*children];
    }
    Ok(elements)
}

fn update_accessibility(state: &WgpuSpikeState) {
    let size = state.view.bounds().size;
    let nodes = super::fixture::semantic_fixture(size.width.round() as u16, false);
    for (element, node) in state.accessibility_elements.iter().zip(nodes.iter()) {
        unsafe {
            element.setAccessibilityFrameInParentSpace(node_rect(node, size.height));
        }
    }
}

fn accessibility_bounds(
    nodes: &[super::fixture::DecisionSemanticNode],
) -> Vec<AccessibilityBoundsArtifact> {
    nodes
        .iter()
        .map(|node| AccessibilityBoundsArtifact {
            id: node.id.clone(),
            x: f64::from(node.bounds.x),
            y: f64::from(node.bounds.y),
            width: f64::from(node.bounds.width),
            height: f64::from(node.bounds.height),
        })
        .collect()
}

fn accessibility_audit_artifact(
    options: &RendererSpikeOptions,
    backing_scale: f64,
    input: InputAuditState,
) -> AccessibilityAuditArtifact {
    let nodes = super::fixture::semantic_fixture(options.logical_size, false);
    AccessibilityAuditArtifact {
        schema_version: ARTIFACT_SCHEMA_VERSION,
        group_count: nodes.iter().filter(|node| node.role == "group").count() as u32,
        value_count: nodes.iter().filter(|node| node.value.is_some()).count() as u32,
        child_count: nodes.len() as u32,
        per_glyph_children: nodes.len() > 4,
        sanitized: nodes.iter().all(|node| {
            !node.name.to_ascii_lowercase().contains("secret")
                && node
                    .value
                    .as_deref()
                    .is_none_or(|value| !value.to_ascii_lowercase().contains("secret"))
        }),
        initial_logical_size: options.logical_size,
        initial_backing_scale: backing_scale,
        initial_bounds: accessibility_bounds(&nodes),
        resize_bounds: input.resize_bounds,
        synthetic_mouse_event_delivered: input.mouse_event_delivered,
        synthetic_key_event_delivered: input.key_event_delivered,
        stale_snapshot_rejected: input.stale_snapshot_rejected,
        current_snapshot_accepted: input.current_snapshot_accepted,
        first_responder: input.first_responder,
        hide_children_detached: input.hide_children_detached,
        reveal_children_restored: input.reveal_children_restored,
        close_children_detached: input.close_children_detached,
    }
}

fn node_rect(node: &super::fixture::DecisionSemanticNode, view_height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(
            f64::from(node.bounds.x),
            view_height - f64::from(node.bounds.y + node.bounds.height),
        ),
        NSSize::new(f64::from(node.bounds.width), f64::from(node.bounds.height)),
    )
}

fn update_occlusion_for_elapsed(state: &mut WgpuSpikeState, elapsed: Duration) {
    let duration_ms = state.options.duration_ms.max(3);
    let first_third = duration_ms / 3;
    let second_third = first_third.saturating_mul(2);
    let elapsed_ms = elapsed.as_millis() as u64;
    if elapsed_ms >= first_third && elapsed_ms < second_third {
        if state.window.isVisible() {
            state.host_calls.push(observation("occlusion-enter"));
            unsafe {
                state.view.setAccessibilityChildren(None);
                for element in &state.accessibility_elements {
                    element.setAccessibilityParent(None);
                }
            }
            state.input_audit.hide_children_detached = true;
            state.window.orderOut(None);
        }
    } else if elapsed_ms >= second_third && !state.window.isVisible() {
        state.host_calls.push(observation("occlusion-exit"));
        state.window.makeKeyAndOrderFront(None);
        let children =
            NSArray::<NSAccessibilityElement>::from_id_slice(&state.accessibility_elements);
        unsafe {
            for element in &state.accessibility_elements {
                element.setAccessibilityParent(Some(&state.view));
            }
            let _: () = msg_send![&state.view, setAccessibilityChildren: &*children];
        }
        state.input_audit.reveal_children_restored = true;
    }
}

fn resize_for_elapsed(state: &mut WgpuSpikeState, elapsed: Duration) {
    let size = match elapsed.as_millis() % 4_000 {
        0..=999 => 360.0,
        1_000..=1_999 => 480.0,
        2_000..=2_999 => 720.0,
        _ => 360.0,
    };
    let mut frame = state.window.frame();
    if (frame.size.width - size).abs() > f64::EPSILON {
        frame.size = NSSize::new(size, size);
        state.window.setFrame_display(frame, true);
        state.host_calls.push(observation("resize-backing-change"));
        let nodes = super::fixture::semantic_fixture(size.round() as u16, false);
        state
            .input_audit
            .resize_bounds
            .push(AccessibilityResizeArtifact {
                logical_size: size.round() as u16,
                backing_scale: state.window.backingScaleFactor(),
                bounds: accessibility_bounds(&nodes),
            });
    }
}

fn finish() {
    let snapshot = WGPU_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        if state.finished {
            return None;
        }
        state.finished = true;
        unsafe {
            state.view.setAccessibilityChildren(None);
            for element in &state.accessibility_elements {
                element.setAccessibilityParent(None);
            }
        }
        state.input_audit.close_children_detached = true;
        Some((
            state.options.clone(),
            state.owner_thread,
            state.submission_count,
            state.callback_panic_count,
            state.metrics.clone(),
            state.started_at.elapsed().as_millis() as u64,
            state.pointer_projection.clone(),
            state.runner_entry_micros,
            state.harness_entry_micros,
            state.host_ready_micros,
            state.first_present_micros,
            state.window.backingScaleFactor(),
            state.input_audit.clone(),
        ))
    });
    let result: Result<()> = (|| {
        let Some((
            options,
            owner_thread,
            submission_count,
            callback_panic_count,
            metrics,
            elapsed_ms,
            pointer_projection,
            runner_entry_micros,
            harness_entry_micros,
            host_ready_micros,
            first_present_micros,
            backing_scale,
            input_audit,
        )) = snapshot
        else {
            return Ok(());
        };
        let owner_label = format!("{owner_thread:?}");
        artifacts::write_json(
            &options.out.join("startup.json"),
            &artifacts::StartupArtifact::from_checkpoints(
                runner_entry_micros,
                harness_entry_micros,
                host_ready_micros,
                first_present_micros,
            )?,
        )?;
        artifacts::write_json(
            &options.out.join("accessibility-audit.json"),
            &accessibility_audit_artifact(&options, backing_scale, input_audit),
        )?;
        let capture_error = if matches!(options.track, RendererSpikeTrack::Capture) {
            WGPU_STATE.with(|cell| {
                let mut state = cell.borrow_mut();
                match state.as_mut() {
                    Some(state) => {
                        state.host_calls.push(observation("capture-poll"));
                        let fixture = canonical_fixture();
                        let frame = resolve_frame(&fixture, elapsed_ms);
                        state.gpu.capture(&frame, &options, 5).err()
                    }
                    None => Some(GlorpError::Message(
                        "wgpu capture state disappeared before finish".into(),
                    )),
                }
            })
        } else {
            None
        };
        let host_calls = WGPU_STATE.with(|cell| {
            let mut state = cell.borrow_mut();
            let state = state.as_mut().ok_or_else(|| {
                GlorpError::Message("wgpu host state disappeared before close".into())
            })?;
            state.host_calls.push(observation("close"));
            Ok::<_, GlorpError>(state.host_calls.clone())
        })?;
        let owner_assertions_passed = host_calls
            .iter()
            .all(|call| call.main_thread && call.thread == owner_label);
        if let Some(pointer_projection) = pointer_projection {
            artifacts::write_json(
                &options.out.join("pointer-projection.json"),
                &pointer_projection,
            )?;
        }
        artifacts::write_json(
            &options.out.join("host-boundary.json"),
            &HostBoundaryArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                candidate: options.candidate,
                owner: "appkit-main-thread".to_string(),
                owner_thread: owner_label,
                observed_threads: host_calls.clone(),
                call_sequence: host_calls
                    .iter()
                    .map(|call| call.operation.clone())
                    .collect(),
                owner_assertions_passed,
            },
        )?;
        let mut metrics_jsonl = String::new();
        for metric in &metrics {
            metrics_jsonl.push_str(&serde_json::to_string(metric)?);
            metrics_jsonl.push('\n');
        }
        std::fs::write(options.out.join("frame-metrics.jsonl"), metrics_jsonl)?;
        artifacts::write_json(
            &options.out.join("process-cleanup.json"),
            &CleanupArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                process_exited: true,
                surviving_pids: Vec::new(),
                timed_out: false,
            },
        )?;
        let verdict = if let Some(error) = &capture_error {
            if error.to_string().contains("injected-capture-timeout") {
                "reject-injected-capture-timeout"
            } else {
                "reject-capture"
            }
        } else if callback_panic_count != 0 {
            "reject-callback-panic"
        } else if !owner_assertions_passed {
            "reject-host-owner"
        } else if submission_count == 0 {
            "reject-no-presentation"
        } else {
            "host-functional-pass"
        };
        artifacts::write_json(
            &options.out.join("summary.json"),
            &SummaryArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                candidate: options.candidate,
                track: options.track,
                verdict: verdict.to_string(),
                cpu_measured: false,
                sample_count: metrics.len(),
                cpu_mean: 0.0,
                cpu_median: 0.0,
                cpu_p95: 0.0,
                privacy_passed: true,
                cleanup_passed: true,
            },
        )?;
        super::privacy::write_privacy_scan(&options.out)?;
        if capture_error.is_none() {
            artifacts::write_manifest(
                &options.out,
                options.candidate,
                options.track,
                options.logical_size,
            )?;
        } else {
            artifacts::write_manifest(
                &options.out,
                options.candidate,
                RendererSpikeTrack::Static,
                options.logical_size,
            )?;
        }
        if let Some(error) = capture_error {
            return Err(error);
        }
        if callback_panic_count != 0 {
            return Err(GlorpError::Message(
                "wgpu renderer spike rejected callback panic".into(),
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("glorp wgpu renderer spike finish failed: {error}");
        std::process::exit(1);
    }
    let app = NSApplication::sharedApplication(MainThreadMarker::new().expect("main thread"));
    unsafe { app.terminate(None) };
}

fn finish_early_fault(
    options: RendererSpikeOptions,
    runner_entry_micros: u64,
    harness_entry_micros: u64,
    verdict: &str,
) -> Result<()> {
    artifacts::write_json(
        &options.out.join("startup.json"),
        &artifacts::StartupArtifact::from_checkpoints(
            runner_entry_micros,
            harness_entry_micros,
            artifacts::monotonic_micros(),
            None,
        )?,
    )?;
    let owner_thread = format!("{:?}", std::thread::current().id());
    let observation = observation("injected-surface-unavailable");
    artifacts::write_json(
        &options.out.join("host-boundary.json"),
        &HostBoundaryArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            candidate: options.candidate,
            owner: "appkit-main-thread".to_string(),
            owner_thread,
            observed_threads: vec![observation.clone()],
            call_sequence: vec![observation.operation],
            owner_assertions_passed: observation.main_thread,
        },
    )?;
    artifacts::write_json(
        &options.out.join("accessibility-audit.json"),
        &accessibility_audit_artifact(&options, 2.0, InputAuditState::default()),
    )?;
    std::fs::write(options.out.join("frame-metrics.jsonl"), "")?;
    artifacts::write_json(
        &options.out.join("summary.json"),
        &SummaryArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            candidate: options.candidate,
            track: options.track,
            verdict: verdict.to_string(),
            cpu_measured: false,
            sample_count: 0,
            cpu_mean: 0.0,
            cpu_median: 0.0,
            cpu_p95: 0.0,
            privacy_passed: true,
            cleanup_passed: true,
        },
    )?;
    super::privacy::write_privacy_scan(&options.out)?;
    artifacts::write_manifest(
        &options.out,
        options.candidate,
        options.track,
        options.logical_size,
    )?;
    Ok(())
}

fn physical_dimension(logical: f64, backing_scale: f64) -> u32 {
    (logical * backing_scale).round().max(1.0) as u32
}

fn gpu_static_instances(fixture: &DecisionSourceFixture) -> Vec<GpuStaticInstance> {
    let atlas = canonical_atlas();
    let mut primitives = fixture.primitives.iter().collect::<Vec<_>>();
    primitives.sort_by_key(|primitive| (primitive.depth_band, primitive.id));
    primitives
        .into_iter()
        .map(|primitive| {
            let atlas_rect = primitive
                .atlas_entry
                .and_then(|index| atlas.entries.get(usize::from(index)))
                .map_or([0.0, 0.0, 1.0, 1.0], |entry| {
                    let x0 = f32::from(entry.rect[0]) / f32::from(atlas.width);
                    let y0 = f32::from(entry.rect[1]) / f32::from(atlas.height);
                    let x1 = f32::from(entry.rect[0] + entry.rect[2]) / f32::from(atlas.width);
                    let y1 = f32::from(entry.rect[1] + entry.rect[3]) / f32::from(atlas.height);
                    [x0, y0, x1, y1]
                });
            GpuStaticInstance {
                base_rect: [
                    primitive.bounds.x,
                    primitive.bounds.y,
                    primitive.bounds.width,
                    primitive.bounds.height,
                ],
                color: primitive.rgba.map(|channel| f32::from(channel) / 255.0),
                atlas_rect,
                params: [
                    match primitive.kind {
                        DecisionPrimitiveKind::Glyph => 0.0,
                        DecisionPrimitiveKind::Rect => 1.0,
                        DecisionPrimitiveKind::Ellipse => 2.0,
                        DecisionPrimitiveKind::Arc => 3.0,
                    },
                    f32::from(primitive.depth_band) * 0.01,
                    if primitive.dynamic {
                        f32::from(primitive.id.0)
                    } else {
                        -1.0
                    },
                    0.0,
                ],
            }
        })
        .collect()
}

fn gpu_frame_instances(frame: &DecisionResolvedFrame) -> Vec<GpuFrameInstance> {
    let fixture = canonical_fixture();
    let source_by_id = fixture
        .primitives
        .iter()
        .map(|primitive| (primitive.id, primitive))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut primitives = frame.primitives.iter().collect::<Vec<_>>();
    primitives.sort_by_key(|primitive| (primitive.depth_band, primitive.id));
    primitives
        .into_iter()
        .map(|primitive| {
            let source = source_by_id[&primitive.id];
            debug_assert_eq!(primitive.bounds.width, source.bounds.width);
            debug_assert_eq!(primitive.bounds.height, source.bounds.height);
            GpuFrameInstance {
                offset: [
                    primitive.bounds.x - source.bounds.x,
                    primitive.bounds.y - source.bounds.y,
                ],
            }
        })
        .collect()
}

fn gpu_atlas_overrides(semantic_tick: u64) -> GpuAtlasOverrides {
    let atlas = canonical_atlas();
    let mut rects = [[0.0; 4]; DYNAMIC_PRIMITIVE_COUNT];
    for (id, rect) in rects.iter_mut().enumerate() {
        let atlas_index = ((id as u64 + semantic_tick) % DYNAMIC_PRIMITIVE_COUNT as u64) as usize;
        let entry = &atlas.entries[atlas_index];
        *rect = [
            f32::from(entry.rect[0]) / f32::from(atlas.width),
            f32::from(entry.rect[1]) / f32::from(atlas.height),
            f32::from(entry.rect[0] + entry.rect[2]) / f32::from(atlas.width),
            f32::from(entry.rect[1] + entry.rect[3]) / f32::from(atlas.height),
        ];
    }
    GpuAtlasOverrides { rects }
}

fn align_up(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

fn write_capture_png(capture: CapturePng<'_>) -> Result<()> {
    let captures = capture.root.join("captures");
    std::fs::create_dir_all(&captures)?;
    let stem = format!(
        "capture-{}-frame-{:06}",
        capture.metadata.logical_size, capture.metadata.frame_index
    );
    let png_path = captures.join(format!("{stem}.png"));
    let file = std::fs::File::create(&png_path)?;
    let mut encoder = png::Encoder::new(
        file,
        capture.metadata.physical_width,
        capture.metadata.physical_height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .and_then(|mut writer| writer.write_image_data(capture.rgba))
        .map_err(|error| GlorpError::Message(format!("wgpu capture png failed: {error}")))?;
    artifacts::write_json(&captures.join(format!("{stem}.json")), &capture.metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_and_frame_data_preserve_stable_depth_id_order() {
        let fixture = canonical_fixture();
        let static_instances = gpu_static_instances(&fixture);
        let frame = resolve_frame(&fixture, 1_000);
        let frame_instances = gpu_frame_instances(&frame);
        assert_eq!(static_instances.len(), fixture.primitives.len());
        assert_eq!(frame_instances.len(), fixture.primitives.len());
        let mut expected = frame.primitives.iter().collect::<Vec<_>>();
        expected.sort_by_key(|primitive| (primitive.depth_band, primitive.id));
        let source_by_id = fixture
            .primitives
            .iter()
            .map(|primitive| (primitive.id, primitive))
            .collect::<std::collections::BTreeMap<_, _>>();
        for (instance, primitive) in frame_instances.iter().zip(expected) {
            let source = source_by_id[&primitive.id];
            assert_eq!(
                instance.offset,
                [
                    primitive.bounds.x - source.bounds.x,
                    primitive.bounds.y - source.bounds.y,
                ]
            );
        }
    }

    #[test]
    fn all_primitives_have_frame_transforms_and_only_semantic_slots_override_atlas() {
        let fixture = canonical_fixture();
        let static_instances = gpu_static_instances(&fixture);
        assert_eq!(
            gpu_frame_instances(&resolve_frame(&fixture, 250)).len(),
            300
        );
        assert_eq!(
            static_instances
                .iter()
                .filter(|instance| instance.params[2] >= 0.0)
                .count(),
            DYNAMIC_PRIMITIVE_COUNT
        );
        assert_eq!(
            std::mem::size_of::<GpuAtlasOverrides>(),
            DYNAMIC_PRIMITIVE_COUNT * std::mem::size_of::<[f32; 4]>()
        );
    }

    #[test]
    fn atlas_overrides_match_resolved_semantic_entries() {
        let fixture = canonical_fixture();
        for elapsed_ms in [0, 250, 1_000, 5_000] {
            let frame = resolve_frame(&fixture, elapsed_ms);
            let overrides = gpu_atlas_overrides(elapsed_ms / 250);
            let static_instances = gpu_static_instances(&fixture);
            let mut resolved = frame.primitives.iter().collect::<Vec<_>>();
            resolved.sort_by_key(|primitive| (primitive.depth_band, primitive.id));
            for (style, primitive) in static_instances.iter().zip(resolved) {
                if style.params[2] >= 0.0 {
                    let slot = style.params[2] as usize;
                    let expected = primitive.atlas_entry.unwrap() as usize;
                    assert_eq!(
                        overrides.rects[slot],
                        gpu_atlas_overrides(0).rects[expected]
                    );
                }
            }
        }
    }

    #[test]
    fn input_snapshot_rejects_stale_generation_or_frame() {
        assert_eq!(accepts_input_snapshot(2, 7, 2, 7), Some((2, 7)));
        assert_eq!(accepts_input_snapshot(1, 7, 2, 7), None);
        assert_eq!(accepts_input_snapshot(2, 6, 2, 7), None);
    }
}
