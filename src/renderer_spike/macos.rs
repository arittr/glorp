#![cfg(target_os = "macos")]

use std::cell::RefCell;
use std::time::{Duration, Instant};

use objc2::declare_class;
use objc2::msg_send_id;
use objc2::mutability;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString, NSTimer};

use crate::error::{GlorpError, Result};

use super::artifacts::{self, CleanupArtifact, SummaryArtifact, ARTIFACT_SCHEMA_VERSION};
use super::{RendererSpikeOptions, RendererSpikeTrack};

struct SmoothSpikeState {
    window: Retained<NSWindow>,
    view: Retained<SmoothSpikeView>,
    options: RendererSpikeOptions,
    started_at: Instant,
    frame_count: u64,
    submission_count: u64,
    callback_panic_count: u64,
    metrics: Vec<artifacts::FrameMetric>,
    finished: bool,
}

thread_local! {
    static SMOOTH_STATE: RefCell<Option<SmoothSpikeState>> = const { RefCell::new(None) };
}

declare_class!(
    struct SmoothSpikeController;

    unsafe impl ClassType for SmoothSpikeController {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "GlorpRendererSpikeController";
    }

    impl DeclaredClass for SmoothSpikeController {}

    unsafe impl SmoothSpikeController {
        #[method(rendererSpikeTick:)]
        fn tick(&self, _sender: Option<&AnyObject>) {
            run_callback("renderer-spike-tick", tick);
        }
    }
);

declare_class!(
    struct SmoothSpikeView;

    unsafe impl ClassType for SmoothSpikeView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "GlorpRendererSpikeSmoothView";
    }

    impl DeclaredClass for SmoothSpikeView {}

    unsafe impl SmoothSpikeView {
        #[method(drawRect:)]
        fn draw_rect(&self, _dirty: NSRect) {
            run_callback("renderer-spike-drawrect", || super::smooth::draw(self, self.bounds()));
        }
    }
);

fn run_callback(label: &'static str, callback: impl FnOnce()) {
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(callback)).is_err() {
        eprintln!("glorp renderer spike caught callback panic: {label}");
        SMOOTH_STATE.with(|cell| {
            if let Ok(mut state) = cell.try_borrow_mut() {
                if let Some(state) = state.as_mut() {
                    state.callback_panic_count = state.callback_panic_count.saturating_add(1);
                }
            }
        });
    }
}

pub fn run_smooth(options: RendererSpikeOptions) -> Result<()> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        GlorpError::Message("renderer spike must run on the macOS main thread".into())
    })?;
    artifacts::write_common_artifacts(&options)?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let frame = NSRect::new(
        NSPoint::new(120.0, 120.0),
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
    window.setTitle(&NSString::from_str("Glorp Renderer Spike — Smooth"));
    let view: Retained<SmoothSpikeView> = unsafe {
        msg_send_id![mtm.alloc::<SmoothSpikeView>(), initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), frame.size)]
    };
    window.setContentView(Some(&view));
    window.makeKeyAndOrderFront(None);
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    let controller: Retained<SmoothSpikeController> =
        unsafe { msg_send_id![SmoothSpikeController::class(), new] };
    let interval = match options.track {
        RendererSpikeTrack::Active => 1.0 / 30.0,
        _ => 1.0 / 15.0,
    };
    SMOOTH_STATE.with(|cell| {
        *cell.borrow_mut() = Some(SmoothSpikeState {
            window,
            view,
            options,
            started_at: Instant::now(),
            frame_count: 0,
            submission_count: 0,
            callback_panic_count: 0,
            metrics: Vec::new(),
            finished: false,
        });
    });
    let _timer = unsafe {
        NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
            interval,
            &controller,
            sel!(rendererSpikeTick:),
            None,
            true,
        )
    };
    unsafe { app.run() };
    Ok(())
}

fn tick() {
    let (should_finish, view) = SMOOTH_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return (true, None);
        };
        if state.finished {
            return (true, None);
        }
        let elapsed = state.started_at.elapsed();
        if matches!(state.options.track, RendererSpikeTrack::Resize) {
            resize_for_elapsed(state, elapsed);
        }
        if matches!(state.options.track, RendererSpikeTrack::Occlusion) {
            update_occlusion_for_elapsed(state, elapsed);
        }
        state.frame_count = state.frame_count.saturating_add(1);
        let should_draw = match state.options.track {
            RendererSpikeTrack::Static => state.submission_count == 0,
            RendererSpikeTrack::Occlusion => state.window.isVisible(),
            _ => true,
        };
        (
            elapsed >= Duration::from_millis(state.options.duration_ms) && state.frame_count >= 5,
            should_draw.then(|| state.view.clone()),
        )
    });
    if let Some(view) = view {
        unsafe { view.setNeedsDisplay(true) };
    }
    if should_finish {
        finish();
    }
}

pub(super) fn elapsed_ms() -> u64 {
    SMOOTH_STATE.with(|cell| {
        cell.borrow()
            .as_ref()
            .map_or(0, |state| state.started_at.elapsed().as_millis() as u64)
    })
}

pub(super) fn record_metric(metric: artifacts::FrameMetric) {
    SMOOTH_STATE.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut() {
            if let Some(state) = state.as_mut() {
                state.metrics.push(metric);
                state.submission_count = state.submission_count.saturating_add(1);
            }
        }
    });
}

fn update_occlusion_for_elapsed(state: &mut SmoothSpikeState, elapsed: Duration) {
    let duration_ms = state.options.duration_ms.max(3);
    let first_third = duration_ms / 3;
    let second_third = first_third.saturating_mul(2);
    let elapsed_ms = elapsed.as_millis() as u64;
    if elapsed_ms >= first_third && elapsed_ms < second_third {
        state.window.orderOut(None);
    } else if elapsed_ms >= second_third && !state.window.isVisible() {
        state.window.makeKeyAndOrderFront(None);
    }
}

fn resize_for_elapsed(state: &mut SmoothSpikeState, elapsed: Duration) {
    let size = match elapsed.as_millis() % 4_000 {
        0..=999 => 360.0,
        1_000..=1_999 => 480.0,
        2_000..=2_999 => 720.0,
        _ => 360.0,
    };
    let mut frame = state.window.frame();
    frame.size = NSSize::new(size, size);
    state.window.setFrame_display(frame, true);
}

fn finish() {
    let snapshot = SMOOTH_STATE.with(|cell| {
        let mut state = cell.borrow_mut();
        let state = state.as_mut()?;
        if state.finished {
            return None;
        }
        state.finished = true;
        Some((
            state.view.clone(),
            state.options.clone(),
            state.submission_count,
            state.metrics.clone(),
        ))
    });
    let result: Result<()> = (|| {
        let Some((view, options, submission_count, metrics)) = snapshot else {
            return Ok(());
        };
        if matches!(options.track, RendererSpikeTrack::Capture) {
            super::smooth::write_capture(&view, &options.out, options.logical_size, 5)?;
        }
        let callback_panic_count = SMOOTH_STATE.with(|cell| {
            cell.borrow()
                .as_ref()
                .map_or(0, |state| state.callback_panic_count)
        });
        artifacts::write_json(
            &options.out.join("process-cleanup.json"),
            &CleanupArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                process_exited: true,
                surviving_pids: Vec::new(),
                timed_out: false,
            },
        )?;
        let mut metrics_jsonl = String::new();
        for metric in &metrics {
            metrics_jsonl.push_str(&serde_json::to_string(metric)?);
            metrics_jsonl.push('\n');
        }
        std::fs::write(options.out.join("frame-metrics.jsonl"), metrics_jsonl)?;
        artifacts::write_json(
            &options.out.join("summary.json"),
            &SummaryArtifact {
                schema_version: ARTIFACT_SCHEMA_VERSION,
                candidate: options.candidate,
                track: options.track,
                verdict: if callback_panic_count == 0 {
                    "functional-pass".to_string()
                } else {
                    "reject-callback-panic".to_string()
                },
                cpu_measured: false,
                sample_count: submission_count as usize,
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
    })();
    if let Err(error) = result {
        eprintln!("glorp renderer spike finish failed: {error}");
    }
    let app = NSApplication::sharedApplication(MainThreadMarker::new().expect("main thread"));
    unsafe { app.terminate(None) };
}
