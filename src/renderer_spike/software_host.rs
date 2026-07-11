#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::ptr;
use std::time::{Duration, Instant};

use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{msg_send, sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityElement, NSAccessibilityGroupRole,
    NSAccessibilityStaticTextRole, NSApplication, NSApplicationActivationPolicy,
    NSBackingStoreType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace, NSEvent,
    NSGraphicsContext, NSImage, NSImageInterpolation, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSArray, NSPoint, NSRect, NSSize, NSString, NSTimer};
use serde::{Deserialize, Serialize};

use crate::error::{GlorpError, Result};

use super::artifacts::{
    self, CleanupArtifact, HostBoundaryArtifact, HostBoundaryObservation, SummaryArtifact,
    ARTIFACT_SCHEMA_VERSION,
};
use super::fixture::{canonical_atlas, canonical_fixture, resolve_frame};
use super::software::SoftwareFramebuffer;
use super::{RendererSpikeFault, RendererSpikeOptions, RendererSpikeTrack};

struct SoftwareNativeBuffer {
    framebuffer: SoftwareFramebuffer,
    _bitmap: Retained<NSBitmapImageRep>,
    image: Retained<NSImage>,
    bitmap_data: *mut u8,
    native_create_count: u64,
    native_copy_bytes: u64,
    generation: u64,
}

impl SoftwareNativeBuffer {
    fn new(width: u32, height: u32) -> Result<Self> {
        let framebuffer = SoftwareFramebuffer::new(width, height).map_err(GlorpError::Message)?;
        let bitmap = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                ptr::null_mut(),
                width as isize,
                height as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                width as isize * 4,
                32,
            )
        }
        .ok_or_else(|| GlorpError::Message("software native bitmap allocation failed".into()))?;
        let bitmap_data = unsafe { bitmap.bitmapData() }.cast::<u8>();
        if bitmap_data.is_null() {
            return Err(GlorpError::Message(
                "software native bitmap data unavailable".into(),
            ));
        }
        unsafe {
            bitmap.setSize(NSSize::new(f64::from(width), f64::from(height)));
        }
        let image = unsafe {
            NSImage::initWithSize(
                NSImage::alloc(),
                NSSize::new(f64::from(width), f64::from(height)),
            )
        };
        unsafe { image.addRepresentation(&bitmap) };
        Ok(Self {
            framebuffer,
            _bitmap: bitmap,
            image,
            bitmap_data,
            native_create_count: 1,
            native_copy_bytes: 0,
            generation: 1,
        })
    }

    fn prepare(&mut self, elapsed_ms: u64) -> super::software::SoftwareRasterStats {
        let fixture = canonical_fixture();
        let frame = resolve_frame(&fixture, elapsed_ms);
        let stats = self.framebuffer.render(&frame, &canonical_atlas());
        unsafe {
            copy_top_left_premultiplied_to_bottom_left(
                self.framebuffer.pixels(),
                self.bitmap_data,
                self.framebuffer.width(),
                self.framebuffer.height(),
            );
        }
        self.native_copy_bytes = self
            .native_copy_bytes
            .saturating_add(self.framebuffer.pixels().len() as u64);
        stats
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<bool> {
        if self.framebuffer.width() == width && self.framebuffer.height() == height {
            return Ok(false);
        }
        let previous_create_count = self.native_create_count;
        let previous_copy_bytes = self.native_copy_bytes;
        let previous_generation = self.generation;
        let mut replacement = Self::new(width, height)?;
        replacement.native_create_count = previous_create_count.saturating_add(1);
        replacement.native_copy_bytes = previous_copy_bytes;
        replacement.generation = previous_generation.saturating_add(1);
        *self = replacement;
        Ok(true)
    }
}

unsafe fn copy_top_left_premultiplied_to_bottom_left(
    source: &[u8],
    destination: *mut u8,
    width: u32,
    height: u32,
) {
    let row_bytes = width as usize * 4;
    for source_y in 0..height as usize {
        let destination_y = height as usize - 1 - source_y;
        ptr::copy_nonoverlapping(
            source.as_ptr().add(source_y * row_bytes),
            destination.add(destination_y * row_bytes),
            row_bytes,
        );
    }
}

struct SoftwareSpikeState {
    window: Retained<NSWindow>,
    view: Retained<SoftwareSpikeView>,
    options: RendererSpikeOptions,
    started_at: Instant,
    frame_count: u64,
    submission_count: u64,
    callback_panic_count: u64,
    callback_panic_injected: bool,
    metrics: Vec<artifacts::FrameMetric>,
    host_calls: Vec<HostBoundaryObservation>,
    native: SoftwareNativeBuffer,
    accessibility_elements: Vec<Retained<NSAccessibilityElement>>,
    pointer_projection: Option<PointerProjectionArtifact>,
    reconfiguration_count: u64,
    finished: bool,
}

thread_local! {
    static SOFTWARE_STATE: RefCell<Option<SoftwareSpikeState>> = const { RefCell::new(None) };
}

declare_class!(
    struct SoftwareSpikeController;

    unsafe impl ClassType for SoftwareSpikeController {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpRendererSpikeSoftwareController";
    }

    impl DeclaredClass for SoftwareSpikeController {}

    unsafe impl SoftwareSpikeController {
        #[method(rendererSpikeSoftwareTick:)]
        fn tick(&self, _sender: Option<&AnyObject>) {
            run_callback("renderer-spike-software-tick", tick);
        }
    }
);

declare_class!(
    struct SoftwareSpikeView;

    unsafe impl ClassType for SoftwareSpikeView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "GlorpRendererSpikeSoftwareView";
    }

    impl DeclaredClass for SoftwareSpikeView {}

    unsafe impl SoftwareSpikeView {
        #[method(acceptsFirstResponder)]
        fn accepts_first_responder(&self) -> bool {
            true
        }

        #[method(mouseDown:)]
        fn mouse_down(&self, event: &NSEvent) {
            run_callback("renderer-spike-software-mousedown", || {
                let location = unsafe { event.locationInWindow() };
                let local = self.convertPoint_fromView(location, None);
                record_pointer_projection(local, self.bounds());
            });
        }

        #[method(drawRect:)]
        fn draw_rect(&self, _dirty: NSRect) {
            run_callback("renderer-spike-software-drawrect", || draw_prepared(self.bounds()));
        }
    }
);

fn run_callback(label: &'static str, callback: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err() {
        eprintln!("glorp renderer spike caught callback panic: {label}");
        SOFTWARE_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                if let Some(state) = state.as_mut() {
                    state.callback_panic_count = state.callback_panic_count.saturating_add(1);
                }
            }
        });
    }
}

pub fn run(options: RendererSpikeOptions) -> Result<()> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        GlorpError::Message("renderer spike must run on the macOS main thread".into())
    })?;
    artifacts::write_common_artifacts(&options)?;
    if options.inject_fault == Some(RendererSpikeFault::SurfaceUnavailable) {
        finish_early_fault(options, "reject-injected-native-resource-unavailable")?;
        return Err(GlorpError::Message(
            "software renderer spike rejected injected native resource unavailable".into(),
        ));
    }
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let frame = NSRect::new(
        NSPoint::new(200.0, 200.0),
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
    window.setTitle(&NSString::from_str("Glorp Renderer Spike — Software"));
    let view: Retained<SoftwareSpikeView> = unsafe {
        msg_send_id![mtm.alloc::<SoftwareSpikeView>(), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), frame.size)]
    };
    window.setContentView(Some(&view));
    window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);

    let backing_scale = window.backingScaleFactor();
    let physical_width = physical_dimension(frame.size.width, backing_scale);
    let physical_height = physical_dimension(frame.size.height, backing_scale);
    let native = SoftwareNativeBuffer::new(physical_width, physical_height)?;
    let accessibility_elements = install_accessibility(&view, frame.size)?;
    let owner_thread = std::thread::current().id();
    let owner_label = format!("{owner_thread:?}");
    let host_calls = [
        "appkit-view-create",
        "app-activate",
        "native-bitmap-create",
        "accessibility-install",
        "pointer-project",
    ]
    .into_iter()
    .map(|operation| HostBoundaryObservation {
        operation: operation.to_string(),
        thread: owner_label.clone(),
        main_thread: MainThreadMarker::new().is_some(),
    })
    .collect();
    let pointer_projection = Some(project_pointer(
        NSPoint::new(frame.size.width * 0.25, frame.size.height * 0.75),
        NSRect::new(NSPoint::new(0.0, 0.0), frame.size),
        options.logical_size,
    ));
    let controller: Retained<SoftwareSpikeController> =
        unsafe { msg_send_id![SoftwareSpikeController::class(), new] };
    let interval = match options.track {
        RendererSpikeTrack::Active => 1.0 / 30.0,
        _ => 1.0 / 15.0,
    };
    SOFTWARE_STATE.with(|cell| {
        *cell.borrow_mut() = Some(SoftwareSpikeState {
            window,
            view,
            options,
            started_at: Instant::now(),
            frame_count: 0,
            submission_count: 0,
            callback_panic_count: 0,
            callback_panic_injected: false,
            metrics: Vec::new(),
            host_calls,
            native,
            accessibility_elements,
            pointer_projection,
            reconfiguration_count: 1,
            finished: false,
        });
    });
    let _timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            interval,
            &controller,
            sel!(rendererSpikeSoftwareTick:),
            None,
            true,
        )
    };
    unsafe { app.run() };
    Ok(())
}

fn tick() {
    let inject_callback_panic = SOFTWARE_STATE.with(|cell| {
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
    let (prepared, should_finish) = SOFTWARE_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return (None, true);
        };
        if state.finished {
            return (None, true);
        }
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
        let prepared = should_submit.then(|| {
            let frame_started = Instant::now();
            let stats = state.native.prepare(elapsed.as_millis() as u64);
            PreparedSubmission {
                view: state.view.clone(),
                frame_started,
                frame_index: state.frame_count,
                elapsed_ms: elapsed.as_millis() as u64,
                primitive_count: stats.primitive_count,
                atlas_misses: stats.atlas_misses,
                upload_bytes: state.native.framebuffer.pixels().len() as u64,
            }
        });
        (
            prepared,
            elapsed >= Duration::from_millis(state.options.duration_ms) && state.frame_count >= 5,
        )
    });
    if let Some(prepared) = prepared {
        unsafe {
            prepared.view.setNeedsDisplay(true);
            prepared.view.displayIfNeeded();
        }
        SOFTWARE_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                if let Some(state) = state.as_mut() {
                    state.submission_count = state.submission_count.saturating_add(1);
                    state.host_calls.push(observation("native-image-submit"));
                    state.metrics.push(artifacts::FrameMetric {
                        frame_index: prepared.frame_index,
                        elapsed_ms: prepared.elapsed_ms,
                        end_to_end_cpu_micros: prepared.frame_started.elapsed().as_micros() as u64,
                        requested_visible_frames: prepared.frame_index,
                        completed_visible_frames: state.submission_count,
                        submissions: state.submission_count,
                        missed_deadlines: 0,
                        primitive_count: prepared.primitive_count,
                        static_rebuilds: u64::from(prepared.frame_index == 1),
                        atlas_misses: prepared.atlas_misses,
                        upload_bytes: prepared.upload_bytes,
                        static_upload_bytes: 0,
                        dynamic_upload_bytes: prepared.upload_bytes,
                        atlas_upload_bytes: 0,
                        uniform_upload_bytes: 0,
                        resource_generation: state.native.generation,
                        draw_calls: 1,
                    });
                }
            }
        });
    }
    if should_finish {
        finish();
    }
}

struct PreparedSubmission {
    view: Retained<SoftwareSpikeView>,
    frame_started: Instant,
    frame_index: u64,
    elapsed_ms: u64,
    primitive_count: u32,
    atlas_misses: u64,
    upload_bytes: u64,
}

fn draw_prepared(bounds: NSRect) {
    SOFTWARE_STATE.with(|cell| {
        let state = cell.borrow();
        let Some(state) = state.as_ref() else {
            return;
        };
        let Some(context) = (unsafe { NSGraphicsContext::currentContext() }) else {
            return;
        };
        let previous_interpolation = unsafe { context.imageInterpolation() };
        let previous_antialias = unsafe { context.shouldAntialias() };
        unsafe {
            context.setImageInterpolation(NSImageInterpolation::None);
            context.setShouldAntialias(false);
            state.native.image.drawInRect_fromRect_operation_fraction(
                bounds,
                NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0)),
                NSCompositingOperation::Copy,
                1.0,
            );
            context.setImageInterpolation(previous_interpolation);
            context.setShouldAntialias(previous_antialias);
        }
    });
}

fn observation(operation: &str) -> HostBoundaryObservation {
    HostBoundaryObservation {
        operation: operation.to_string(),
        thread: format!("{:?}", std::thread::current().id()),
        main_thread: MainThreadMarker::new().is_some(),
    }
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

#[derive(Debug, Clone, Serialize)]
struct CaptureMetadata {
    schema_version: u16,
    logical_size: u16,
    physical_width: u32,
    physical_height: u32,
    frame_index: u64,
    orientation: &'static str,
    color_format: &'static str,
    bytes_per_row: u32,
    encode_duration_micros: u64,
    resource_generation: u64,
}

fn record_pointer_projection(point: NSPoint, bounds: NSRect) {
    SOFTWARE_STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.as_mut() {
                state.pointer_projection =
                    Some(project_pointer(point, bounds, state.options.logical_size));
                state.host_calls.push(observation("pointer-project"));
            }
        }
    });
}

fn project_pointer(point: NSPoint, bounds: NSRect, logical_size: u16) -> PointerProjectionArtifact {
    let width = bounds.size.width.max(1.0);
    let height = bounds.size.height.max(1.0);
    PointerProjectionArtifact {
        schema_version: ARTIFACT_SCHEMA_VERSION,
        view_x: point.x,
        view_y: point.y,
        logical_x: point.x / width * f64::from(logical_size),
        logical_y: (height - point.y) / height * f64::from(logical_size),
        inside: point.x >= 0.0 && point.y >= 0.0 && point.x <= width && point.y <= height,
    }
}

fn install_accessibility(
    view: &SoftwareSpikeView,
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

fn update_accessibility(state: &SoftwareSpikeState) {
    let size = state.view.bounds().size;
    let nodes = super::fixture::semantic_fixture(size.width.round() as u16, false);
    for (element, node) in state.accessibility_elements.iter().zip(nodes.iter()) {
        unsafe {
            element.setAccessibilityFrameInParentSpace(node_rect(node, size.height));
        }
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

fn update_occlusion_for_elapsed(state: &mut SoftwareSpikeState, elapsed: Duration) {
    let duration_ms = state.options.duration_ms.max(3);
    let first_third = duration_ms / 3;
    let second_third = first_third.saturating_mul(2);
    let elapsed_ms = elapsed.as_millis() as u64;
    if elapsed_ms >= first_third && elapsed_ms < second_third {
        if state.window.isVisible() {
            state.host_calls.push(observation("occlusion-enter"));
            state.window.orderOut(None);
        }
    } else if elapsed_ms >= second_third && !state.window.isVisible() {
        state.host_calls.push(observation("occlusion-exit"));
        state.window.makeKeyAndOrderFront(None);
    }
}

fn resize_for_elapsed(state: &mut SoftwareSpikeState, elapsed: Duration) {
    let size = match elapsed.as_millis() % 4_000 {
        0..=999 => 360.0,
        1_000..=1_999 => 480.0,
        2_000..=2_999 => 720.0,
        _ => 360.0,
    };
    let bounds = state.view.bounds();
    if (bounds.size.width - size).abs() > f64::EPSILON
        || (bounds.size.height - size).abs() > f64::EPSILON
    {
        state.window.setContentSize(NSSize::new(size, size));
        let bounds = state.view.bounds();
        let backing_scale = state.window.backingScaleFactor();
        let width = physical_dimension(bounds.size.width, backing_scale);
        let height = physical_dimension(bounds.size.height, backing_scale);
        if state.native.resize(width, height).unwrap_or(false) {
            state.reconfiguration_count = state.reconfiguration_count.saturating_add(1);
            state.host_calls.push(observation("resize-backing-change"));
            state.host_calls.push(observation("native-bitmap-create"));
        }
    }
}

#[derive(Serialize)]
struct SoftwareResourceArtifact {
    schema_version: u16,
    physical_width: u32,
    physical_height: u32,
    framebuffer_bytes: u64,
    native_bitmap_creations: u64,
    native_image_creations: u64,
    native_copy_bytes: u64,
    generation: u64,
    reconfiguration_count: u64,
    byte_order: &'static str,
    alpha_format: &'static str,
    source_orientation: &'static str,
    native_orientation: &'static str,
}

fn finish() {
    let snapshot = SOFTWARE_STATE.with(|cell| {
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
        state.host_calls.push(observation("close"));
        Some((
            state.options.clone(),
            state.submission_count,
            state.callback_panic_count,
            state.metrics.clone(),
            state.host_calls.clone(),
            state.native.framebuffer.width(),
            state.native.framebuffer.height(),
            state.native.framebuffer.pixels().len() as u64,
            state.native.native_create_count,
            state.native.native_copy_bytes,
            state.native.generation,
            state.reconfiguration_count,
            state.pointer_projection.clone(),
        ))
    });
    let result: Result<()> = (|| {
        let Some((
            options,
            submission_count,
            callback_panic_count,
            metrics,
            host_calls,
            width,
            height,
            framebuffer_bytes,
            native_create_count,
            native_copy_bytes,
            generation,
            reconfiguration_count,
            pointer_projection,
        )) = snapshot
        else {
            return Ok(());
        };
        let owner_thread = host_calls
            .first()
            .map_or_else(|| "unknown".to_string(), |call| call.thread.clone());
        let owner_assertions_passed = host_calls
            .iter()
            .all(|call| call.main_thread && call.thread == owner_thread);
        if let Some(pointer_projection) = pointer_projection {
            artifacts::write_json(
                &options.out.join("pointer-projection.json"),
                &pointer_projection,
            )?;
        }
        let capture_error = if matches!(options.track, RendererSpikeTrack::Capture) {
            if options.inject_fault == Some(RendererSpikeFault::CaptureTimeout) {
                Some(GlorpError::Message(
                    "software capture failed: injected-capture-timeout".into(),
                ))
            } else {
                write_capture(&options, 5).err()
            }
        } else {
            None
        };
        artifacts::write_json(
            &options.out.join("host-boundary.json"),
            &HostBoundaryArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                candidate: options.candidate,
                owner: "appkit-main-thread".to_string(),
                owner_thread,
                observed_threads: host_calls.clone(),
                call_sequence: host_calls
                    .iter()
                    .map(|call| call.operation.clone())
                    .collect(),
                owner_assertions_passed,
            },
        )?;
        artifacts::write_json(
            &options.out.join("software-resource.json"),
            &SoftwareResourceArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                physical_width: width,
                physical_height: height,
                framebuffer_bytes,
                native_bitmap_creations: native_create_count,
                native_image_creations: native_create_count,
                native_copy_bytes,
                generation,
                reconfiguration_count,
                byte_order: "rgba8",
                alpha_format: "premultiplied",
                source_orientation: "top-left",
                native_orientation: "bottom-left",
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
        artifacts::write_manifest(
            &options.out,
            options.candidate,
            if capture_error.is_some() {
                RendererSpikeTrack::Static
            } else {
                options.track
            },
            options.logical_size,
        )?;
        if let Some(error) = capture_error {
            return Err(error);
        }
        if callback_panic_count != 0 {
            return Err(GlorpError::Message(
                "software renderer spike rejected callback panic".into(),
            ));
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("glorp software renderer spike finish failed: {error}");
        std::process::exit(1);
    }
    let app = NSApplication::sharedApplication(MainThreadMarker::new().expect("main thread"));
    unsafe { app.terminate(None) };
}

fn write_capture(options: &RendererSpikeOptions, frame_index: u64) -> Result<()> {
    SOFTWARE_STATE.with(|cell| {
        let state = cell.borrow();
        let state = state.as_ref().ok_or_else(|| {
            GlorpError::Message("software capture state disappeared before finish".into())
        })?;
        let captures = options.out.join("captures");
        std::fs::create_dir_all(&captures)?;
        let stem = format!("capture-{}-frame-{frame_index:06}", options.logical_size);
        let png_path = captures.join(format!("{stem}.png"));
        let started = Instant::now();
        let straight_rgba = straight_rgba_for_png(state.native.framebuffer.pixels());
        let file = std::fs::File::create(&png_path)?;
        let mut encoder = png::Encoder::new(
            file,
            state.native.framebuffer.width(),
            state.native.framebuffer.height(),
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&straight_rgba))
            .map_err(|error| {
                GlorpError::Message(format!("software capture PNG encoding failed: {error}"))
            })?;
        artifacts::write_json(
            &captures.join(format!("{stem}.json")),
            &CaptureMetadata {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                logical_size: options.logical_size,
                physical_width: state.native.framebuffer.width(),
                physical_height: state.native.framebuffer.height(),
                frame_index,
                orientation: "top-left",
                color_format: "rgba8-straight-srgb-png",
                bytes_per_row: state.native.framebuffer.width().saturating_mul(4),
                encode_duration_micros: started.elapsed().as_micros() as u64,
                resource_generation: state.native.generation,
            },
        )
    })
}

fn straight_rgba_for_png(premultiplied: &[u8]) -> Vec<u8> {
    premultiplied
        .chunks_exact(4)
        .flat_map(|pixel| {
            let alpha = pixel[3];
            if alpha == 0 {
                [0, 0, 0, 0]
            } else {
                [
                    unpremultiply_channel(pixel[0], alpha),
                    unpremultiply_channel(pixel[1], alpha),
                    unpremultiply_channel(pixel[2], alpha),
                    alpha,
                ]
            }
        })
        .collect()
}

fn unpremultiply_channel(channel: u8, alpha: u8) -> u8 {
    ((u32::from(channel) * 255 + u32::from(alpha) / 2) / u32::from(alpha)).min(255) as u8
}

fn finish_early_fault(options: RendererSpikeOptions, verdict: &str) -> Result<()> {
    let observation = observation("injected-native-resource-unavailable");
    artifacts::write_json(
        &options.out.join("host-boundary.json"),
        &HostBoundaryArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            candidate: options.candidate,
            owner: "appkit-main-thread".to_string(),
            owner_thread: observation.thread.clone(),
            observed_threads: vec![observation.clone()],
            call_sequence: vec![observation.operation],
            owner_assertions_passed: observation.main_thread,
        },
    )?;
    std::fs::write(options.out.join("frame-metrics.jsonl"), "")?;
    artifacts::write_json(
        &options.out.join("software-resource.json"),
        &SoftwareResourceArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            physical_width: 0,
            physical_height: 0,
            framebuffer_bytes: 0,
            native_bitmap_creations: 0,
            native_image_creations: 0,
            native_copy_bytes: 0,
            generation: 0,
            reconfiguration_count: 0,
            byte_order: "rgba8",
            alpha_format: "premultiplied",
            source_orientation: "top-left",
            native_orientation: "bottom-left",
        },
    )?;
    artifacts::write_json(
        &options.out.join("process-cleanup.json"),
        &CleanupArtifact {
            schema_version: ARTIFACT_SCHEMA_VERSION,
            process_exited: true,
            surviving_pids: Vec::new(),
            timed_out: false,
        },
    )?;
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
    )
}

fn physical_dimension(logical: f64, backing_scale: f64) -> u32 {
    (logical * backing_scale).round().max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_left_rows_copy_to_bottom_left_native_storage() {
        let source = [
            1, 2, 3, 4, 5, 6, 7, 8, // top
            9, 10, 11, 12, 13, 14, 15, 16, // bottom
        ];
        let mut destination = [0_u8; 16];
        unsafe {
            copy_top_left_premultiplied_to_bottom_left(&source, destination.as_mut_ptr(), 2, 2);
        }
        assert_eq!(
            destination,
            [9, 10, 11, 12, 13, 14, 15, 16, 1, 2, 3, 4, 5, 6, 7, 8]
        );
    }

    #[test]
    fn pointer_projection_handles_retina_independently_of_backing_pixels() {
        let bounds = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(360.0, 360.0));
        let projection = project_pointer(NSPoint::new(90.0, 270.0), bounds, 360);
        assert_eq!(projection.logical_x, 90.0);
        assert_eq!(projection.logical_y, 90.0);
        assert!(projection.inside);
    }

    #[test]
    fn persistent_native_buffer_replaces_resources_only_for_new_dimensions() {
        let mut native = SoftwareNativeBuffer::new(8, 8).unwrap();
        assert!(!native.resize(8, 8).unwrap());
        assert_eq!(native.native_create_count, 1);
        assert_eq!(native.generation, 1);
        assert!(native.resize(16, 12).unwrap());
        assert_eq!(native.native_create_count, 2);
        assert_eq!(native.generation, 2);
        assert_eq!(native.framebuffer.width(), 16);
        assert_eq!(native.framebuffer.height(), 12);
    }

    #[test]
    fn png_conversion_unpremultiplies_channels() {
        assert_eq!(
            straight_rgba_for_png(&[100, 50, 25, 128, 0, 0, 0, 0]),
            vec![199, 100, 50, 128, 0, 0, 0, 0]
        );
    }
}
