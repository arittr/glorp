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
//!    `NSBitmapImageRep::initWithBitmapDataPlanes...` use in `resources.rs` is a
//!    different selector and is deliberately not in scope here.

use std::fs;
use std::path::{Path, PathBuf};

const RETAINED_DELETION_INVENTORY: &[&str] = &[
    "src/companion/paired_review.rs",
    "src/companion/retained.rs",
    "src/companion/retained.wgsl",
    "src/companion/retained/buffers.rs",
    "src/companion/retained/capture.rs",
    "src/companion/retained/compiler.rs",
    "src/companion/retained/host.rs",
    "src/companion/retained/hud.rs",
    "src/companion/retained/metrics.rs",
    "src/companion/retained/parity.rs",
    "src/companion/retained/presentation.rs",
    "src/companion/retained/render.rs",
    "src/companion/retained/resources.rs",
    "src/companion/retained/scene.wgsl",
    "src/companion/retained/worker.rs",
];

fn collect_files(dir: &Path, root: &Path, files: &mut Vec<String>) {
    for entry in
        fs::read_dir(dir).unwrap_or_else(|error| panic!("cannot list {}: {error}", dir.display()))
    {
        let path = entry.expect("source inventory entry is readable").path();
        if path.is_dir() {
            collect_files(&path, root, files);
        } else {
            files.push(
                path.strip_prefix(root)
                    .expect("inventory path is under repository root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

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

#[test]
fn retained_deletion_inventory_is_complete_and_intentional() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut actual = vec![
        "src/companion/paired_review.rs".to_string(),
        "src/companion/retained.rs".to_string(),
        "src/companion/retained.wgsl".to_string(),
    ];
    collect_files(&root.join("src/companion/retained"), root, &mut actual);
    actual.sort();

    let mut expected = RETAINED_DELETION_INVENTORY
        .iter()
        .map(|path| (*path).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(
        actual, expected,
        "retained cutover/deletion inventory changed; classify every added, moved, or removed \
         retained file explicitly rather than silently weakening the boundary"
    );

    for scaffold in [
        "src/companion/paired_review.rs",
        "src/companion/retained.rs",
    ] {
        assert!(RETAINED_DELETION_INVENTORY.contains(&scaffold));
        assert!(
            root.join(scaffold).is_file(),
            "missing scaffold: {scaffold}"
        );
    }
}

#[test]
fn native_accessibility_and_input_survive_retained_view_transitions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let app = read(&root.join("src/companion/app.rs"));
    let host = read(&root.join("src/companion/retained/host.rs"));

    for callback in [
        "fn is_accessibility_element(&self) -> bool",
        "fn accessibility_role(&self) -> Retained<NSString>",
        "fn accessibility_label(&self) -> Retained<NSString>",
        "fn accessibility_value(&self) -> Retained<NSString>",
        "fn accessibility_frame(&self) -> NSRect",
    ] {
        assert!(
            app.contains(callback),
            "missing RoundView semantic callback: {callback}"
        );
    }
    assert!(app.contains("NSString::from_str(\"AXGroup\")"));
    assert!(app.contains("NSString::from_str(\"Glorp habitat\")"));
    assert!(
        app.contains("window.convertRectToScreen(self.convertRect_toView(self.bounds(), None))")
    );

    assert!(host.contains("view.setLayer(Some(&self.host.layer))"));
    assert!(host.contains(".setDrawableSize(NSSize::new(f64::from(width), f64::from(height)))"));
    assert!(host.contains("view.setLayer(None)"));
    assert_eq!(app.matches("window.setContentView(Some(&view))").count(), 1);

    for input_contract in [
        "Some(sel!(terminate:))",
        "NSCommandKeyMask",
        "Some(sel!(toggleFullScreen:))",
        "NSControlKeyMask.0 | NSCommandKeyMask.0",
        "NSWindowStyleMask::Closable",
        "NSWindowStyleMask::Miniaturizable",
        "NSWindowStyleMask::Resizable",
        "window.setMovableByWindowBackground(true)",
    ] {
        assert!(
            app.contains(input_contract),
            "native input contract lost: {input_contract}"
        );
    }
}

#[test]
fn reduce_motion_is_refreshed_dynamically_and_gates_scene_projection() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/app.rs");
    let app = read(&path);
    assert!(app.contains("accessibilityDisplayShouldReduceMotion()"));
    assert!(app.contains("state.reduce_motion = reduce_motion"));
    let projection = function_body_from(&app, "fn prepare_scene_runtime_tick()");
    assert_eq!(projection.matches("reduce_motion,").count(), 2);
    assert_eq!(
        projection.matches("CompanionPresentationOptions {").count(),
        2
    );
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read retained source {}: {error}", path.display()))
}

fn production_source(text: &str) -> &str {
    text.split("\n#[cfg(test)]\nmod tests {")
        .next()
        .unwrap_or(text)
}

fn function_body_from<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing {signature}"));
    let tail = &source[start..];
    let open = tail.find('{').expect("function body opens");
    let mut depth = 0_u32;
    for (offset, byte) in tail[open..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &tail[..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body for {signature}");
}

#[test]
fn retained_host_owns_appkit_surface_and_queue_types() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/retained");
    let host_path = root.join("host.rs");
    let host = read(&host_path);
    for required in [
        "surface: wgpu::Surface<'static>,",
        "config: wgpu::SurfaceConfiguration,",
        "device: wgpu::Device,",
        "queue: wgpu::Queue,",
        "layer: Retained<CAMetalLayer>,",
    ] {
        assert!(
            host.contains(required),
            "retained host must own {required} in {}",
            host_path.display(),
        );
    }

    // `parity.rs` has isolated headless GPU test infrastructure. The production
    // ownership seam covers the live retained root and its runtime support
    // modules. Helpers may borrow Device/Queue, while buffers borrow Device to
    // allocate storage; only host.rs may own the live surface stack as fields.
    let mut ownership_sources = vec![root.with_extension("rs")];
    for relative in [
        "buffers.rs",
        "capture.rs",
        "metrics.rs",
        "presentation.rs",
        "resources.rs",
        "worker.rs",
    ] {
        ownership_sources.push(root.join(relative));
    }
    for path in ownership_sources {
        let text = read(&path);
        let production = production_source(&text);
        for forbidden in [
            "CAMetalLayer",
            "surface: wgpu::Surface<'static>",
            "config: wgpu::SurfaceConfiguration",
            "device: wgpu::Device",
            "queue: wgpu::Queue",
            "MainThreadMarker",
        ] {
            assert!(
                !production.contains(forbidden),
                "live retained ownership outside host.rs must not name {forbidden}: {}",
                path.display(),
            );
        }
    }
}

#[test]
fn retained_buffers_have_no_appkit_or_objc2_dependencies() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/retained/buffers.rs");
    let text = read(&path);
    for forbidden in [
        "AppKit",
        "objc2",
        "NSView",
        "CAMetalLayer",
        "MainThreadMarker",
        "device: wgpu::Device",
        "queue: wgpu::Queue",
    ] {
        assert!(
            !text.contains(forbidden),
            "retained buffers must remain platform-UI-free ({forbidden} in {})",
            path.display(),
        );
    }
}

#[test]
fn retained_scene_render_contract_uses_reusable_uploads_and_owns_its_color_math() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/retained/render.rs");
    let text = read(&path);
    let production = production_source(&text);
    for forbidden in [
        "device: wgpu::Device",
        "queue: wgpu::Queue",
        "queue.write_buffer",
        "remove_srgb_suffix",
        "premultiply_gamma_srgb",
        "srgb_channel_to_linear",
        "linear_channel_to_srgb",
        "parity::",
    ] {
        assert!(
            !production.contains(forbidden),
            "scene render contract must borrow GPU state, preserve the reusable staging-belt \
             upload invariant, and keep independent IEC color math; \
             found {forbidden} in {}",
            path.display(),
        );
    }
}

#[test]
fn retained_habitat_presentation_does_not_own_renderer_lifecycle_policy() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let presentation_sources = [
        "src/presentation/props.rs",
        "src/presentation/companion_effects.rs",
        "src/presentation/tank_life.rs",
        "src/presentation/companion_scene/composition.rs",
        "src/presentation/companion_scene/input.rs",
        "src/presentation/companion_scene/scene.rs",
        "src/presentation/companion_scene/scene/compiler.rs",
        "src/presentation/companion_scene/scene/checksum.rs",
        "src/presentation/companion_scene/validate.rs",
    ];
    let forbidden_owners = [
        "wgpu::",
        "CAMetalLayer",
        "SurfaceConfiguration",
        "MainThreadMarker",
        "NSRunLoop",
        "EventLoop",
        "NSTimer",
        "NSWindow",
        "toggleFullScreen",
        "windowDidResize",
        "resize_surface",
        "RendererChoice",
        "renderer_choice",
        "SceneRuntimeRollout",
        "RetainedFailureCategory",
        "fallback_to_smooth",
    ];

    for relative in presentation_sources {
        let source = read(&root.join(relative));
        let production = production_source(&source);
        for forbidden in forbidden_owners {
            assert!(
                !production.contains(forbidden),
                "presentation/composition must stay platform-neutral and cannot own renderer, \
                 surface, event-loop, fallback, resize, or fullscreen policy; found \
                 {forbidden:?} in {relative}"
            );
        }
    }
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
fn direct_live_capture_uses_scene_readback_and_emits_the_required_evidence_set() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let app = read(&root.join("src/companion/app.rs"));
    let direct = read(&root.join("src/companion/direct_capture.rs"));
    let host = read(&root.join("src/companion/retained/host.rs"));

    let finish = function_body_from(&app, "fn finish_review_capture_if_due()");
    assert!(finish.contains("should_run_direct_terminal_capture("));
    assert!(finish.contains("run_direct_scene_capture(state)"));
    assert!(
        !finish.contains("renderer_runtime.effective().is_retained()"),
        "a requested Live qualification must fail through direct capture rather than bypass it after fallback"
    );
    let terminal_route = function_body_from(&app, "fn should_run_direct_terminal_capture(");
    assert!(terminal_route.contains("renderer.is_retained()"));
    assert!(terminal_route.contains("scene_runtime_rollout == SceneRuntimeRollout::Live"));
    let capture = function_body_from(&app, "fn run_direct_scene_capture(");
    assert!(capture.contains(".capture_presented_scene(sensitive)"));
    assert!(capture.contains("write_direct_scene_capture("));
    assert!(!capture.contains("last_good_frame"));

    let service = function_body_from(&app, "fn service_live_scene_runtime(");
    assert!(service.contains("ActivationTransition::RetryLater"));
    assert!(service.contains("capture.record_offscreen_review_tick()"));

    let host_capture = function_body_from(&host, "fn capture_presented_scene_inner(");
    assert!(host_capture.contains("last_presented_scene"));
    assert!(host_capture.contains("capture_safe_clone()"));
    assert!(host_capture.contains("render_offscreen("));
    assert!(host_capture.contains("render_offscreen_sensitive("));
    for forbidden in [
        "bitmapImageRepForCachingDisplayInRect",
        "cacheDisplayInRect_toBitmapImageRep",
        "SmoothCompanionScenePlan",
    ] {
        assert!(
            !host_capture.contains(forbidden),
            "direct capture cannot consume AppKit or legacy Smooth evidence: {forbidden}"
        );
    }

    for artifact in [
        "scene.png",
        "scene-manifest.json",
        "scene-snapshot.json",
        "scene-version.json",
        "scene-metrics.json",
    ] {
        assert!(
            direct.contains(artifact),
            "missing direct artifact {artifact}"
        );
    }
    assert!(direct.contains("validate_direct_rgba("));
    assert!(direct.contains("direct-retained-scene"));
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
        "terminal snapshot must be emitted after paired capture records its attempted/succeeded/failed outcome"
    );
}

#[test]
fn activation_finishes_on_first_successful_present_not_frame_zero() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/retained/host.rs");
    let text = read(&path);
    assert!(text.contains("activation_render_owner_us"));
    assert!(
        !text.contains("activation_excluded_appkit_us"),
        "activation no longer subtracts AppKit time because raster work is outside render"
    );
    assert!(text.contains("!self.activation_recorded"));
    assert!(text.contains("progress.disposition() == Some(FrameDisposition::SurfacePresentCalled)"));
    assert!(!text.contains("(self.frame_counter == 0).then(Instant::now)"));
}

#[test]
fn live_and_legacy_ticks_keep_preparation_out_of_presentation_render() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = read(&root.join("src/companion/retained/host.rs"));
    let render_start = host
        .find("    pub(super) fn render(")
        .or_else(|| host.find("    pub(in crate::companion) fn render("))
        .expect("retained render exists");
    let render_tail = &host[render_start..];
    let render_end = render_tail
        .find("\n    ///")
        .expect("render has a following method");
    let render = &render_tail[..render_end];
    for forbidden in [
        "ensure_resources(",
        "CompiledRetainedResources::compile(",
        "rasterize_glyph_entry(",
        "advance_resource_preparation(",
    ] {
        assert!(
            !render.contains(forbidden),
            "presentation render must not perform AppKit preparation: {forbidden}"
        );
    }

    let app = read(&root.join("src/companion/app.rs"));
    let tick = function_body_from(&app, "fn ui_tick()");
    let hidden = tick
        .find("if !companion_view_is_visible()")
        .expect("hidden early return exists");
    let live = tick
        .find("scene_runtime_rollout == SceneRuntimeRollout::Live")
        .expect("Live route exists");
    let live_tail = &tick[live..];
    let live_animate = live_tail
        .find("animate_pet()")
        .expect("Live animation exists");
    let live_prepare = live_tail
        .find("prepare_scene_runtime_tick()")
        .expect("Live snapshot preparation exists");
    let live_service = live_tail
        .find("service_scene_runtime(&tick)")
        .expect("Live scene service exists");
    let live_return = live_tail.find("return;").expect("Live route returns");
    let live_body = &live_tail[..live_return];
    assert!(hidden < live && live_animate < live_prepare && live_prepare < live_service);
    for forbidden in [
        "present_retained_active_generation()",
        "drive_retained_resource_preparation()",
        "prepare_current_frame_from_state()",
    ] {
        assert!(
            !live_body.contains(forbidden),
            "Live route must not enter legacy preparation: {forbidden}"
        );
    }

    let legacy = &tick[live + live_return..];
    let active_present = legacy
        .find("present_retained_active_generation()")
        .expect("legacy active generation presentation exists");
    let worker_service = legacy
        .find("drive_retained_resource_preparation()")
        .expect("legacy visible tick services the raster worker");
    let animate = legacy
        .find("animate_pet()")
        .expect("legacy animation exists");
    let present = legacy
        .find("prepare_current_frame_from_state()")
        .expect("legacy presentation preparation exists");
    assert!(
        active_present < worker_service && worker_service < animate && worker_service < present
    );
    assert!(legacy.contains("ResourcePreparationTick::Yielded"));

    let drive_start = app
        .find("fn drive_retained_resource_preparation()")
        .expect("preparation driver exists");
    let drive = &app[drive_start..];
    let resize = drive
        .find("backing_scale_for_resource_preparation")
        .expect("backing scale observation exists");
    let advance = drive
        .find("advance_resource_preparation")
        .expect("resource preparation advances");
    assert!(
        resize < advance,
        "backing scale must be current before request matching"
    );
}

#[test]
fn host_owned_worker_validates_completed_resources_before_atomic_publish() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = read(&root.join("src/companion/retained/host.rs"));
    assert!(text.contains("scene_build_worker: SceneBuildWorker"));
    let start = text
        .find("    fn advance_resource_preparation(\n        &mut self,")
        .expect("retained resource preparation method exists");
    let tail = &text[start..];
    let end = tail
        .find("\n    fn record_resource_preparation_skip(")
        .expect("skip recorder follows worker lifecycle methods");
    let body = &tail[..end];
    let poll = body
        .find(".try_recv()")
        .expect("worker polling is nonblocking");
    let validate = body
        .find("let still_current = self.resource_preparation.visible")
        .expect("completed result is fully revalidated");
    let materialize = body
        .find("self.publish_prepared_resources(request, resources)")
        .expect("validated result reaches materialization");
    let upload = body
        .find("upload_glyph_atlas(")
        .expect("complete atlas uploads");
    let publish = body
        .find("self.glyph_resources = Some(")
        .expect("complete generation publishes");
    assert!(poll < validate && validate < materialize && materialize < upload && upload < publish);
    for required in [
        "finish_running(reply_id)",
        "request == &current",
        "accepts_completed(request, &desired_key)",
        "desired.as_ref() == Some(&request)",
        "ResourcePreparationKey::new(identity.clone(), desired_backing_scale)",
    ] {
        assert!(
            body.contains(required),
            "missing stale-result gate: {required}"
        );
    }
    assert!(!body.contains("self.glyph_resources.take()"));

    let worker = read(&root.join("src/companion/retained/worker.rs"));
    for forbidden in [
        "wgpu",
        "Device",
        "Queue",
        "upload_glyph_atlas",
        "materialize",
    ] {
        assert!(
            !worker.contains(forbidden),
            "CPU raster worker must not own GPU materialization: {forbidden}"
        );
    }
}

#[test]
fn production_and_evidence_paths_never_use_monolithic_appkit_atlas_compile() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for relative in [
        "src/companion/retained.rs",
        "src/companion/retained/host.rs",
        "src/companion/paired_review.rs",
        "src/companion/app.rs",
    ] {
        let text = read(&root.join(relative));
        let production = text
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .unwrap_or(&text);
        assert!(
            !production.contains("CompiledRetainedResources::compile("),
            "{relative} must not bypass the host-owned raster worker"
        );
        assert!(!production.contains("CompiledRetainedResourcesPreparation"));
    }
}

#[test]
fn scene_generation_service_timing_excludes_render_owner_materialization() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = read(&root.join("src/companion/retained/host.rs"));
    let body = function_body_from(
        &host,
        "pub(in crate::companion) fn advance_scene_generation(",
    );
    let poll = body
        .find("self.host.advance_scene_generation()")
        .expect("the UI service polls once");
    let service_metric = body
        .find("record_generation_service_ui_us")
        .expect("the bounded service slice is recorded");
    let materialize = body
        .find("self.host.materialize_scene_candidate()")
        .expect("GPU materialization is a separate render-owner phase");
    assert!(poll < service_metric && service_metric < materialize);
}

#[test]
fn scene_surface_switch_occurs_only_after_activation_can_start() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let host = read(&root.join("src/companion/retained/host.rs"));
    let start = host
        .find("    pub(super) fn activate_candidate(")
        .expect("scene activation transaction exists");
    let tail = &host[start..];
    let end = tail
        .find("\n    #[allow(dead_code)] // Reached through the dormant Task 12 entrypoint above.\n    fn finish_scene_activation")
        .expect("activation finisher bounds the transaction");
    let body = &tail[..end];
    let begin = body
        .find("let attempt = match generations.begin_activation()")
        .expect("runtime authority starts the candidate");
    let configure = body
        .find("self.surface.configure(&self.device, &scene_config)")
        .expect("the host switches to the scene surface");
    assert!(begin < configure);
}

#[test]
fn atlas_fonts_resolve_through_nullable_bounded_boundary_once_per_job() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let resources = read(&root.join("src/companion/retained/resources.rs"));

    assert!(resources.contains("const FONT_RESOLUTION_MAX_ATTEMPTS: usize = 3;"));
    assert!(resources.contains("msg_send_id!["));
    assert!(resources.contains("monospacedSystemFontOfSize: point_size,"));
    assert!(resources.contains("weight: weight.ns_weight()"));
    assert!(resources.contains("Option<Retained<NSFont>>"));
    assert!(resources.contains("autoreleasepool(|_| resolver(point_size, weight))"));

    let preparation_start = resources.find("struct GlyphAtlasPreparation {").unwrap();
    let preparation_end = resources[preparation_start..].find("\n}").unwrap() + preparation_start;
    let preparation = &resources[preparation_start..preparation_end];
    assert!(preparation.contains("fonts: ResolvedAtlasFonts"));
    assert!(resources.contains("regular: Retained<NSFont>"));
    assert!(resources.contains("bold: Retained<NSFont>"));

    let raster_start = resources.find("fn rasterize_glyph_entry_impl(").unwrap();
    let raster_end = resources[raster_start..]
        .find("\n/// Inner drawable box")
        .unwrap()
        + raster_start;
    let raster = &resources[raster_start..raster_end];
    assert!(raster.contains("font: &NSFont"));
    assert!(!raster.contains("resolve_font_once("));
    assert!(!raster.contains("monospacedSystemFontOfSize"));

    let presentation = read(&root.join("src/companion/retained/presentation.rs"));
    assert!(presentation.contains("FontUnavailable"));
    assert!(presentation.contains("retained-font-unavailable"));
}

#[test]
fn instance_uploads_use_one_host_owned_staging_belt_on_every_submission_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let retained = read(&root.join("src/companion/retained.rs"));
    let production = production_source(&retained);
    let host = read(&root.join("src/companion/retained/host.rs"));
    let host_production = production_source(&host);
    let buffers = read(&root.join("src/companion/retained/buffers.rs"));
    for (path, source) in [
        ("retained.rs", production),
        ("host.rs", host_production),
        ("buffers.rs", buffers.as_str()),
    ] {
        assert!(
            !source.contains("queue.write_buffer"),
            "instance uploads must use the reusable staging belt in {path}"
        );
    }
    let buffers_start = buffers.find("struct PersistentFrameBuffers {").unwrap();
    let buffers_end = buffers[buffers_start..].find("\n}").unwrap() + buffers_start;
    assert_eq!(
        buffers[buffers_start..buffers_end]
            .matches("staging_belt: wgpu::util::StagingBelt")
            .count(),
        1
    );
    assert!(buffers.contains("FIXED_INSTANCE_RING_MIN * INSTANCE_STRIDE"));

    let render_start = host_production
        .find("    pub(in crate::companion) fn render(")
        .unwrap();
    let render = &host_production[render_start..];
    let acquire = render.find("self.surface.get_current_texture()").unwrap();
    let stage = render
        .find("self.prepare_frame(&mut encoder, &frame)")
        .unwrap();
    let finish = render.find("self.frame_buffers.finish_uploads()").unwrap();
    let submit = render
        .find("self.queue.submit([encoder.finish()])")
        .unwrap();
    let recall = render.find("self.frame_buffers.recall_uploads()").unwrap();
    assert!(acquire < stage && stage < finish && finish < submit && submit < recall);

    let scene_render = read(&root.join("src/companion/retained/render.rs"));
    let direct = function_body_from(&scene_render, "fn submit_active_offscreen_inner(");
    let stage = direct.find("encode_scene_delta_copies(").unwrap();
    let finish = direct.find("self.staging_belt.finish()").unwrap();
    let submit = direct.find("queue.submit([command])").unwrap();
    let recall = direct.find("self.staging_belt.recall()").unwrap();
    assert!(stage < finish && finish < submit && submit < recall);

    let capture = read(&root.join("src/companion/retained/capture.rs"));
    assert!(
        capture.find("prepare_frame(&mut encoder").unwrap()
            < capture.find("finish_uploads()").unwrap()
    );
    assert!(capture.find("finish_uploads()").unwrap() < capture.find("queue.submit").unwrap());
    assert!(capture.find("queue.submit").unwrap() < capture.find("recall_uploads()").unwrap());
}

#[test]
fn retained_startup_waits_for_first_visibility_guarded_worker_service() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/app.rs");
    let text = read(&path);
    let state_ready = text
        .find("*cell.borrow_mut() = Some(AppState {")
        .expect("AppState installation exists");
    let timer = text[state_ready..]
        .find("NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats")
        .map(|offset| state_ready + offset)
        .expect("UI timer follows AppState installation");
    let startup = &text[state_ready..timer];
    assert!(
        !startup.contains("drive_retained_resource_preparation()"),
        "retained raster work must begin only inside visibility-guarded ui_tick"
    );
    let timer_setup = &text[timer..];
    assert!(timer_setup.contains("addTimer_forMode(&timer, NSRunLoopCommonModes)"));
    assert!(timer_setup.contains("APP_TIMER.with(|slot|"));
}

#[test]
fn scene_lifecycle_uses_host_epochs_common_modes_and_explicit_shutdown() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let app = read(&root.join("src/companion/app.rs"));
    let shutdown = function_body_from(&app, "fn shutdown_app_lifecycle()");
    let invalidate = shutdown.find("timer.invalidate()").unwrap();
    let runtime_shutdown = shutdown.find("host.shutdown_scene_runtime()").unwrap();
    let restore = shutdown.find("restore_appkit").unwrap();
    assert!(invalidate < runtime_shutdown && runtime_shutdown < restore);

    let hidden = function_body_from(&app, "fn hide_scene_runtime(");
    assert!(hidden.contains("host.hide_scene_runtime()?"));
    assert!(!hidden.contains("prepare_scene_runtime_tick"));
    assert!(!hidden.contains("CompanionSceneSnapshot::project"));

    let retained = read(&root.join("src/companion/retained.rs"));
    let production = production_source(&retained);
    assert!(production.contains("acknowledge_operational_surface_rebound_to(surface)"));
    assert!(!production.contains("acknowledge_operational_surface_rebound()"));

    let runtime = read(&root.join("src/presentation/companion_scene/runtime.rs"));
    let operational_allocator = runtime
        .find("pub(crate) fn acknowledge_operational_surface_rebound(")
        .expect("test-only compatibility allocator exists");
    assert!(runtime[..operational_allocator].ends_with("#[cfg(test)]\n    "));
}

#[test]
fn active_hold_precedes_worker_service_and_no_active_records_explicit_skip() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/companion/app.rs");
    let text = read(&path);
    let tick_start = text.find("fn ui_tick()").expect("ui_tick exists");
    let tick_tail = &text[tick_start..];
    let tick_end = tick_tail
        .find("\n/// After a runtime fallback")
        .expect("ui_tick end marker exists");
    let tick = &tick_tail[..tick_end];
    let worker_service = tick
        .find("drive_retained_resource_preparation()")
        .expect("worker service is driven");
    let hold = tick
        .find("present_retained_active_generation()")
        .expect("active generation presentation exists");
    assert!(hold < worker_service);
    assert!(tick.contains("ResourcePreparationTick::YieldedWithoutActive"));

    let drive_start = text
        .find("fn drive_retained_resource_preparation()")
        .expect("preparation driver exists");
    let drive = &text[drive_start..];
    assert!(drive.contains("record_resource_preparation_skip()"));
}
