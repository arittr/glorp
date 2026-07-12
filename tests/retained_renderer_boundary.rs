//! Source-boundary guard for the retained companion renderer.
//!
//! This test is a pure text scan (not feature-gated), so it runs in the default
//! build config. It enforces two ownership boundaries the canonical readback
//! rests on:
//!
//! 1. No retained production source may reference `renderer_spike::` — the GPU
//!    readback is *ported* into production types, never imported from the
//!    prototype. If it were imported, the paired-review parity evidence would
//!    depend on spike code the cutover decision is not supposed to trust.
//! 2. The retained capture file must not fall back to AppKit view-caching
//!    (`bitmapImageRepForCachingDisplayInRect` /
//!    `cacheDisplayInRect_toBitmapImageRep`) to obtain pixels — capture must
//!    read them straight off the GPU. The glyph rasterizer's unrelated
//!    `NSBitmapImageRep::initWithBitmapDataPlanes...` use in `retained.rs` is a
//!    different selector and is deliberately not in scope here.

use std::fs;
use std::path::{Path, PathBuf};

/// Every retained production source file the `renderer_spike::` ban covers:
/// `retained.rs`, every module file under `retained/`, and `paired_review.rs`.
fn retained_source_files() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = vec![
        root.join("src/companion/retained.rs"),
        root.join("src/companion/paired_review.rs"),
    ];
    let retained_dir = root.join("src/companion/retained");
    for entry in fs::read_dir(&retained_dir).unwrap_or_else(|error| {
        panic!(
            "cannot list retained module dir {}: {error}",
            retained_dir.display()
        )
    }) {
        let path = entry.expect("retained module dir entry is readable").path();
        if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read retained source {}: {error}", path.display()))
}

#[test]
fn retained_sources_never_reference_the_renderer_spike() {
    let files = retained_source_files();
    // Guard against a silently empty scan (e.g. a moved module).
    assert!(
        files.len() >= 3,
        "expected at least retained.rs, paired_review.rs, and one retained/ module",
    );
    for path in files {
        let text = read(&path);
        assert!(
            !text.contains("renderer_spike::"),
            "retained production source {} must not reference renderer_spike:: — \
             the GPU readback is ported into production types, not imported",
            path.display(),
        );
    }
}

#[test]
fn retained_capture_never_falls_back_to_appkit_view_caching() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/retained/capture.rs");
    let text = read(&path);
    for selector in [
        "bitmapImageRepForCachingDisplayInRect",
        "cacheDisplayInRect_toBitmapImageRep",
    ] {
        assert!(
            !text.contains(selector),
            "retained capture must read pixels off the GPU, not via AppKit \
             view-caching ({selector} in {})",
            path.display(),
        );
    }
}

#[test]
fn draw_scene_reads_only_last_good_frame_and_never_records_runtime_metrics() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/app.rs");
    let text = read(&path);
    let start = text.find("\nfn draw_scene(").expect("draw_scene exists");
    let tail = &text[start..];
    let end = tail
        .find("\nfn paint_prepared_frame(")
        .expect("paint_prepared_frame follows draw_scene");
    let body = &tail[..end];
    assert!(body.contains("state.last_good_frame.as_ref()"));
    for forbidden in [
        "prepare_current_frame_from_state(",
        "prepare_companion_frame(",
        "runtime_metrics",
        "record_ui_tick_us(",
        "record_prepare_us(",
        "record_encode_us(",
    ] {
        assert!(
            !body.contains(forbidden),
            "draw_scene must remain a last-good-frame consumer; found {forbidden}"
        );
    }
}

#[test]
fn terminal_metrics_survive_fallback_and_follow_paired_capture() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/app.rs");
    let text = read(&path);
    assert!(
        text.contains("terminal_runtime_metrics"),
        "fallback must preserve a terminal runtime snapshot after host teardown"
    );
    let finish = text
        .find("fn finish_review_capture_if_due()")
        .expect("finish_review_capture_if_due exists");
    let tail = &text[finish..];
    let capture = tail
        .find("run_paired_capture(state)")
        .expect("paired capture runs");
    let snapshot = tail
        .find("write_runtime_metrics_if_requested(state)")
        .expect("terminal metrics are written");
    assert!(
        capture < snapshot,
        "terminal snapshot must be emitted after paired capture increments capture_count"
    );
}

#[test]
fn activation_finishes_on_first_successful_present_not_frame_zero() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/retained.rs");
    let text = read(&path);
    assert!(text.contains("activation_render_owner_us"));
    assert!(text.contains("activation_excluded_appkit_us"));
    assert!(text.contains("!self.activation_recorded"));
    assert!(text.contains("progress.disposition() == Some(FrameDisposition::SurfacePresentCalled)"));
    assert!(!text.contains("(self.frame_counter == 0).then(Instant::now)"));
}
