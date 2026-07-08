use crate::dev_preview::export::{
    PreviewDimensions, PreviewPixelArtArtifact, PreviewPixelFitArtifact,
    PreviewPixelFitGeometryEvidence, PreviewPixelFrameArtifact, PreviewPixelHudOverlap,
    PreviewPlayback, PreviewScenarioKind, PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles,
    PreviewStripKind, PIXEL_ART_SCHEMA_VERSION, PIXEL_FIT_SCHEMA_VERSION,
    PIXEL_FRAME_SCHEMA_VERSION,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::{PreviewRenderContext, PreviewScenarioBundle};
use crate::dev_preview::strips::PreviewStripBundle;
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::{
    art::stage_label, generation::generate_pet, generation::Species, render::render_pet,
};
use crate::presentation::pixel::{
    render_pixel_frame, PixelArtReferenceProvider, PixelBounds, PixelFrame, PixelPetArtReference,
    PixelPetInput, PixelPetScene, PixelRendererState, PixelRendererTick, PixelViewport,
};
use crate::round::hud::companion_hud_text;
use crate::round::pixel_fit::{pixel_companion_fit, PixelCompanionFit, PixelTargetGeometry};
use crate::tui::view_model::WatchViewModel;
use ratatui::{buffer::Buffer, layout::Rect, style::Style};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type PreviewPixelBundle = PreviewScenarioBundle;
pub type PreviewPixelStripBundle = PreviewStripBundle;

const FRAME_DURATION_MS: u16 = 34;
const STRIP_FRAME_COUNT: usize = 48;
const STRIP_SPAN_MS: u16 = 1_600;
const PREVIEW_FIT_MIN_TARGET_SIZE: u16 = 260;
const PREVIEW_FIT_TARGET_SIZE: u16 = 360;
const PREVIEW_FIT_LARGE_TARGET_SIZE: u16 = 480;
const PREVIEW_FIT_FULLSCREEN_TARGET_SIZE: u16 = 900;

struct PixelPreviewArtifacts {
    frame: PreviewPixelFrameArtifact,
    art: PreviewPixelArtArtifact,
    fit: PreviewPixelFitArtifact,
    fit_status_lines: Vec<String>,
}

pub fn pixel_bundles(ctx: &PreviewRenderContext) -> Vec<PreviewPixelBundle> {
    vec![
        render_pixel_bundle(
            ctx,
            PixelFixture {
                id: "pixel-fuzz-s3-content-idle",
                title: "Pixel Fuzz S3 Content Idle",
                species: Species::Fuzz,
                stage: Stage::S3,
                mood: Mood::Content,
                asleep: false,
                calm: false,
                burst_level: 0.0,
                pulse_age_ms: None,
                elapsed_ms: 480,
            },
            &["species fuzz", "stage s3 pup", "mood content", "pose idle"],
            "Review the companion pixel renderer in a stable awake idle pose.",
        ),
        render_pixel_bundle(
            ctx,
            PixelFixture {
                id: "pixel-glitch-s4-feed-pulse",
                title: "Pixel Glitch S4 Feed Pulse",
                species: Species::Glitch,
                stage: Stage::S4,
                mood: Mood::Content,
                asleep: false,
                calm: false,
                burst_level: 0.9,
                pulse_age_ms: Some(300),
                elapsed_ms: 300,
            },
            &["species glitch", "stage s4 shardglitch", "mood content", "pulse feed"],
            "Review the companion pixel renderer with a live feed pulse and glitch accents.",
        ),
        render_pixel_bundle(
            ctx,
            PixelFixture {
                id: "pixel-species-matrix",
                title: "Pixel Species Matrix",
                species: Species::Crystal,
                stage: Stage::S5,
                mood: Mood::Happy,
                asleep: false,
                calm: false,
                burst_level: 0.35,
                pulse_age_ms: None,
                elapsed_ms: 720,
            },
            &["fuzz blob ghost", "glitch crystal mech", "anchor crystal s5", "palette survey"],
            "Review a representative pixel companion frame alongside the species roster used for pixel fixture coverage.",
        ),
    ]
}

pub fn pixel_strips(ctx: &PreviewRenderContext) -> Vec<PreviewPixelStripBundle> {
    vec![
        render_pixel_strip(
            ctx,
            PixelFixture {
                id: "pixel-idle",
                title: "Pixel Idle",
                species: Species::Fuzz,
                stage: Stage::S3,
                mood: Mood::Content,
                asleep: false,
                calm: false,
                burst_level: 0.0,
                pulse_age_ms: None,
                elapsed_ms: 0,
            },
            "Shows awake idle motion with blink beats in the portable pixel renderer.",
        ),
        render_pixel_strip(
            ctx,
            PixelFixture {
                id: "pixel-asleep-calm",
                title: "Pixel Asleep Calm",
                species: Species::Blob,
                stage: Stage::S2,
                mood: Mood::Sleepy,
                asleep: true,
                calm: true,
                burst_level: 0.0,
                pulse_age_ms: None,
                elapsed_ms: 0,
            },
            "Shows the lower-amplitude asleep breathing loop in the portable pixel renderer.",
        ),
        render_pixel_strip(
            ctx,
            PixelFixture {
                id: "pixel-feed-pulse",
                title: "Pixel Feed Pulse",
                species: Species::Glitch,
                stage: Stage::S4,
                mood: Mood::Content,
                asleep: false,
                calm: false,
                burst_level: 0.95,
                pulse_age_ms: Some(0),
                elapsed_ms: 0,
            },
            "Shows the feed pulse animation sweep in the portable pixel renderer.",
        ),
    ]
}

#[derive(Clone, Copy)]
struct PixelFixture {
    id: &'static str,
    title: &'static str,
    species: Species,
    stage: Stage,
    mood: Mood,
    asleep: bool,
    calm: bool,
    burst_level: f32,
    pulse_age_ms: Option<u16>,
    elapsed_ms: u16,
}

fn render_pixel_bundle(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    lines: &[&str],
    intent: &'static str,
) -> PreviewPixelBundle {
    let (artifacts, input, request) = render_pixel_artifact(ctx, fixture, fixture.elapsed_ms);
    let vm = fixture_view_model(fixture, ctx.fixed_now);
    let dimensions = PreviewDimensions {
        width: artifacts.frame.width,
        height: artifacts.frame.height,
    };
    let mut summary_lines = lines
        .iter()
        .map(|line| (*line).to_string())
        .collect::<Vec<_>>();
    summary_lines.extend(artifacts.fit_status_lines.clone());
    summary_lines.push("terminal reference".to_string());
    summary_lines.extend(render_terminal_reference_lines(&request));
    let mut frame = summary_frame(fixture.id, fixture.title, &summary_lines);
    frame.contract.pixel = Some(artifacts.frame);
    frame.contract.pixel_art = Some(artifacts.art);
    frame.contract.pixel_fit = Some(artifacts.fit);

    PreviewScenarioBundle::from_parts_with_dimensions(
        frame,
        PreviewScenarioKind::Pixel,
        intent,
        dimensions,
        scenario_inputs(&input, &vm, fixture.elapsed_ms),
        None,
        Vec::new(),
    )
}

fn render_pixel_strip(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    intent: &'static str,
) -> PreviewPixelStripBundle {
    let mut frames = Vec::with_capacity(STRIP_FRAME_COUNT);
    let mut manifest_frames = Vec::with_capacity(STRIP_FRAME_COUNT);
    let mut reference_provider = PixelArtReferenceProvider::default();

    for index in 0..STRIP_FRAME_COUNT {
        let elapsed_ms = elapsed_for_index(index);
        let (artifacts, input, _) = render_pixel_artifact_with_provider(
            ctx,
            fixture,
            elapsed_ms,
            ctx.fixed_now,
            &mut reference_provider,
        );
        let scene = pixel_scene_for_elapsed(&input, elapsed_ms);
        let mut frame = strip_placeholder_frame(
            &format!("{}-frame-{index:03}", fixture.id),
            &phase_for_scene(&input, scene),
        );
        frame.contract.pixel = Some(artifacts.frame);
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: index as u16,
            phase: phase_for_scene(&input, scene),
            elapsed_ms,
            files: pixel_strip_frame_paths(fixture.id, index),
        });
    }

    PreviewStripBundle {
        manifest: PreviewStrip {
            id: fixture.id.to_string(),
            kind: PreviewStripKind::PixelAnimation,
            title: fixture.title.to_string(),
            intent: intent.to_string(),
            dimensions: PreviewDimensions { width: 96, height: 96 },
            target_id: "companion.pixel.pet".to_string(),
            playback: PreviewPlayback {
                starts_paused: true,
                frame_duration_ms: FRAME_DURATION_MS,
            },
            inputs: scenario_inputs(
                &fixture_input(fixture, ctx.fixed_now),
                &fixture_view_model(fixture, ctx.fixed_now),
                STRIP_SPAN_MS,
            ),
            frames: manifest_frames,
            review_prompts: Vec::new(),
        },
        frames,
    }
}

fn render_pixel_artifact(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    elapsed_ms: u16,
) -> (
    PixelPreviewArtifacts,
    PixelPetInput,
    crate::presentation::pixel::PixelArtReferenceRequest,
) {
    let now = ctx.fixed_now + time::Duration::milliseconds(i64::from(elapsed_ms));
    render_pixel_artifact_with_pulse_anchor(ctx, fixture, elapsed_ms, now)
}

fn render_pixel_artifact_with_pulse_anchor(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    elapsed_ms: u16,
    pulse_anchor: time::OffsetDateTime,
) -> (
    PixelPreviewArtifacts,
    PixelPetInput,
    crate::presentation::pixel::PixelArtReferenceRequest,
) {
    let mut reference_provider = PixelArtReferenceProvider::default();
    render_pixel_artifact_with_provider(
        ctx,
        fixture,
        elapsed_ms,
        pulse_anchor,
        &mut reference_provider,
    )
}

fn render_pixel_artifact_with_provider(
    ctx: &PreviewRenderContext,
    fixture: PixelFixture,
    elapsed_ms: u16,
    pulse_anchor: time::OffsetDateTime,
    reference_provider: &mut PixelArtReferenceProvider,
) -> (
    PixelPreviewArtifacts,
    PixelPetInput,
    crate::presentation::pixel::PixelArtReferenceRequest,
) {
    let base = ctx.fixed_now;
    let now = base + time::Duration::milliseconds(i64::from(elapsed_ms));
    let vm = fixture_view_model(fixture, pulse_anchor);
    let (input, request) = PixelPetInput::from_watch_view_model_with_art_request(&vm, now);
    let art_reference = reference_provider.reference_for(&request);
    let mut state = PixelRendererState::new(&input, base);
    let frame = render_pixel_frame(PixelRendererTick {
        input: &input,
        art_reference: &art_reference,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    });
    (
        PixelPreviewArtifacts {
            frame: pixel_artifact(&frame, &input, elapsed_ms),
            art: pixel_art_sidecar(&input, &art_reference),
            fit: pixel_fit_sidecar(&frame, &vm),
            fit_status_lines: render_fit_status_lines(&frame, &vm),
        },
        input,
        request,
    )
}

fn fixture_view_model(fixture: PixelFixture, pulse_anchor: time::OffsetDateTime) -> WatchViewModel {
    let mut vm = WatchViewModel::fixture();
    vm.pet_render.generated_species = fixture.species;
    vm.pet_render.stage = fixture.stage;
    vm.pet_render.mood = fixture.mood;
    vm.stage = stage_label(fixture.species, fixture.stage).to_string();
    vm.mood = fixture.mood.as_str().to_string();
    vm.life_profile.calm_mode = fixture.calm;
    vm.day_context.asleep = fixture.asleep;
    vm.life_profile.burst_level = fixture.burst_level;
    vm.last_feed_pulse_at = fixture
        .pulse_age_ms
        .map(|age_ms| pulse_anchor - time::Duration::milliseconds(i64::from(age_ms)));
    vm
}

fn fixture_input(fixture: PixelFixture, now: time::OffsetDateTime) -> PixelPetInput {
    let vm = fixture_view_model(fixture, now);
    PixelPetInput::from_watch_view_model(&vm, now)
}

fn scenario_inputs(
    input: &PixelPetInput,
    vm: &WatchViewModel,
    elapsed_ms: u16,
) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "renderer".to_string(),
            Value::String("portable-pixel".to_string()),
        ),
        (
            "species".to_string(),
            Value::String(input.identity.species.as_str().to_string()),
        ),
        (
            "stage".to_string(),
            Value::String(input.identity.stage.as_str().to_string()),
        ),
        (
            "mood".to_string(),
            Value::String(input.mood.as_str().to_string()),
        ),
        ("elapsed_ms".to_string(), json!(elapsed_ms)),
        ("asleep".to_string(), json!(input.sleep.asleep)),
        ("calm".to_string(), json!(input.sleep.calm)),
        ("pulse_active".to_string(), json!(input.pulse.active)),
        ("fit".to_string(), preview_fit_input(vm)),
    ])
}

fn preview_fit_input(vm: &WatchViewModel) -> Value {
    let viewport = PixelViewport::companion_default();
    let hud = companion_hud_text(
        vm.today_effective_tokens,
        vm.daily_comparison.fraction_of_yesterday,
        vm.rate_momentum.pulse.current_tokens,
    );
    let fit = pixel_companion_fit(default_preview_fit_geometry(), viewport, &hud);

    json!({
        "producer": fit.producer,
        "geometry": {
            "width": fit.geometry.width,
            "height": fit.geometry.height,
        },
        "viewport": {
            "logical_width": viewport.logical_width,
            "logical_height": viewport.logical_height,
        },
        "scale": fit.scale,
        "image_rect": preview_fit_rect_json(fit.image_rect),
        "hud_safe_zone": preview_fit_rect_json(fit.hud_safe_zone),
    })
}

fn preview_fit_rect_json(rect: crate::round::pixel_fit::PixelFitRect) -> Value {
    json!({
        "x": rect.x,
        "y": rect.y,
        "width": rect.width,
        "height": rect.height,
    })
}

fn default_preview_fit_geometry() -> PixelTargetGeometry {
    PixelTargetGeometry {
        width: PREVIEW_FIT_TARGET_SIZE,
        height: PREVIEW_FIT_TARGET_SIZE,
    }
}

fn fit_geometries() -> [(&'static str, PixelTargetGeometry); 4] {
    [
        (
            "min",
            PixelTargetGeometry {
                width: PREVIEW_FIT_MIN_TARGET_SIZE,
                height: PREVIEW_FIT_MIN_TARGET_SIZE,
            },
        ),
        ("default", default_preview_fit_geometry()),
        (
            "large",
            PixelTargetGeometry {
                width: PREVIEW_FIT_LARGE_TARGET_SIZE,
                height: PREVIEW_FIT_LARGE_TARGET_SIZE,
            },
        ),
        (
            "fullscreen",
            PixelTargetGeometry {
                width: PREVIEW_FIT_FULLSCREEN_TARGET_SIZE,
                height: PREVIEW_FIT_FULLSCREEN_TARGET_SIZE,
            },
        ),
    ]
}

fn render_fit_status_lines(frame: &PixelFrame, vm: &WatchViewModel) -> Vec<String> {
    let viewport = PixelViewport::companion_default();
    let hud = companion_hud_text(
        vm.today_effective_tokens,
        vm.daily_comparison.fraction_of_yesterday,
        vm.rate_momentum.pulse.current_tokens,
    );
    fit_geometries()
        .into_iter()
        .map(|(label, geometry)| {
            let fit = pixel_companion_fit(geometry, viewport, &hud);
            let body_overlap = hud_overlap_pixels(frame, &fit, |alpha| alpha >= 200);
            let effect_overlap = hud_overlap_pixels(frame, &fit, |alpha| alpha > 0 && alpha < 200);
            format!(
                "fit {label} {}",
                if body_overlap == 0 && effect_overlap == 0 {
                    "ready"
                } else {
                    "check"
                }
            )
        })
        .collect()
}

fn render_terminal_reference_lines(
    request: &crate::presentation::pixel::PixelArtReferenceRequest,
) -> Vec<String> {
    render_pet(
        &generate_pet(&request.seed).with_species(request.species),
        request.stage,
        request.mood,
        request.animation_frame,
    )
    .lines
    .into_iter()
    .collect()
}

fn summary_frame(id: &str, title: &str, lines: &[String]) -> PreviewFrame {
    let width = lines
        .iter()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(24)
        .max(24) as u16;
    let height = lines.len().max(1) as u16;
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
    for (row, line) in lines.iter().enumerate() {
        buffer.set_string(0, row as u16, line, Style::default());
    }
    frame_from_buffer(id, title, &buffer)
}

fn strip_placeholder_frame(id: &str, phase: &str) -> PreviewFrame {
    let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
    buffer.set_string(0, 0, phase, Style::default());
    frame_from_buffer(id, phase, &buffer)
}

fn elapsed_for_index(index: usize) -> u16 {
    ((u32::from(STRIP_SPAN_MS) * index as u32) / (STRIP_FRAME_COUNT.saturating_sub(1) as u32))
        as u16
}

fn pixel_scene_for_elapsed(input: &PixelPetInput, elapsed_ms: u16) -> PixelPetScene {
    PixelPetScene::from_elapsed_ms(input, i64::from(elapsed_ms))
}

fn phase_for_scene(input: &PixelPetInput, scene: PixelPetScene) -> String {
    if scene.blink_closed {
        "blink-closed".to_string()
    } else if input.sleep.asleep {
        "asleep-calm".to_string()
    } else if input.pulse.active {
        "feed-pulse".to_string()
    } else {
        "idle".to_string()
    }
}

fn pixel_strip_frame_paths(strip_id: &str, index: usize) -> PreviewStripFrameFiles {
    PreviewStripFrameFiles {
        text: PathBuf::from(format!("strips/{strip_id}/frame-{index:03}.txt")),
        cells: PathBuf::from(format!("strips/{strip_id}/frame-{index:03}.cells.json")),
        pixel: Some(PathBuf::from(format!(
            "strips/{strip_id}/frame-{index:03}.pixel.json"
        ))),
    }
}

fn pixel_artifact(
    frame: &PixelFrame,
    input: &PixelPetInput,
    elapsed_ms: u16,
) -> PreviewPixelFrameArtifact {
    PreviewPixelFrameArtifact {
        schema_version: PIXEL_FRAME_SCHEMA_VERSION,
        width: frame.width,
        height: frame.height,
        elapsed_ms,
        species: input.identity.species.as_str().to_string(),
        stage: input.identity.stage.as_str().to_string(),
        mood: input.mood.as_str().to_string(),
        pixels: frame
            .pixels
            .iter()
            .map(|p| format!("#{:02x}{:02x}{:02x}{:02x}", p.r, p.g, p.b, p.a))
            .collect(),
    }
}

fn pixel_art_sidecar(
    input: &PixelPetInput,
    reference: &PixelPetArtReference,
) -> PreviewPixelArtArtifact {
    PreviewPixelArtArtifact {
        schema_version: PIXEL_ART_SCHEMA_VERSION,
        species: input.identity.species.as_str().to_string(),
        stage: input.identity.stage.as_str().to_string(),
        mood: input.mood.as_str().to_string(),
        reference_checksum: format!("{:016x}", reference.reference_checksum.0),
        width_cells: reference.width_cells,
        height_cells: reference.height_cells,
        body_bounds: reference.body_bounds,
        foot_contact: reference.foot_contact.clone(),
        role_counts: reference.role_counts.clone(),
    }
}

fn pixel_fit_sidecar(frame: &PixelFrame, vm: &WatchViewModel) -> PreviewPixelFitArtifact {
    let viewport = PixelViewport::companion_default();
    let hud = companion_hud_text(
        vm.today_effective_tokens,
        vm.daily_comparison.fraction_of_yesterday,
        vm.rate_momentum.pulse.current_tokens,
    );
    let fit = pixel_companion_fit(default_preview_fit_geometry(), viewport, &hud);
    let geometry_evidence = fit_geometry_evidence(frame, viewport, &hud);

    PreviewPixelFitArtifact {
        schema_version: PIXEL_FIT_SCHEMA_VERSION,
        producer: fit.producer,
        geometry: fit.geometry,
        image_rect: fit.image_rect,
        hud_safe_zone: fit.hud_safe_zone,
        hud_overlap: PreviewPixelHudOverlap {
            body_eye_mouth_pixels: hud_overlap_pixels(frame, &fit, |alpha| alpha >= 200),
            translucent_effect_pixels: hud_overlap_pixels(frame, &fit, |alpha| {
                alpha > 0 && alpha < 200
            }),
        },
        geometry_evidence,
    }
}

fn fit_geometry_evidence(
    frame: &PixelFrame,
    viewport: PixelViewport,
    hud: &crate::round::hud::CompanionHudText,
) -> Vec<PreviewPixelFitGeometryEvidence> {
    fit_geometries()
        .into_iter()
        .map(|(label, geometry)| {
            let fit = pixel_companion_fit(geometry, viewport, hud);
            PreviewPixelFitGeometryEvidence {
                label,
                producer: fit.producer,
                geometry: fit.geometry,
                image_rect: fit.image_rect,
                hud_safe_zone: fit.hud_safe_zone,
                hud_overlap: PreviewPixelHudOverlap {
                    body_eye_mouth_pixels: hud_overlap_pixels(frame, &fit, |alpha| alpha >= 200),
                    translucent_effect_pixels: hud_overlap_pixels(frame, &fit, |alpha| {
                        alpha > 0 && alpha < 200
                    }),
                },
            }
        })
        .collect()
}

fn hud_overlap_pixels(
    frame: &PixelFrame,
    fit: &PixelCompanionFit,
    include_alpha: impl Fn(u8) -> bool,
) -> u16 {
    let mut count = 0_u16;
    for y in 0..frame.height {
        for x in 0..frame.width {
            let idx = usize::from(y) * usize::from(frame.width) + usize::from(x);
            let alpha = frame.pixels[idx].a;
            if !include_alpha(alpha) {
                continue;
            }
            let bounds = PixelBounds { min_x: x, min_y: y, max_x: x, max_y: y };
            if fit.map_logical_bounds(bounds).overlaps(fit.hud_safe_zone) {
                count = count.saturating_add(1);
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dev_preview::scenarios::PreviewRenderContext;

    #[test]
    fn pixel_strip_reference_provider_reuses_cached_pose_during_sequence() {
        let ctx = PreviewRenderContext::deterministic();
        let fixture = PixelFixture {
            id: "pixel-idle",
            title: "Pixel Idle",
            species: Species::Fuzz,
            stage: Stage::S3,
            mood: Mood::Content,
            asleep: false,
            calm: false,
            burst_level: 0.0,
            pulse_age_ms: None,
            elapsed_ms: 0,
        };

        let render_count = strip_reference_render_count_for_test(&ctx, fixture);

        assert!(
            render_count < STRIP_FRAME_COUNT,
            "expected strip rendering to reuse cached art references, got {render_count} renders for {STRIP_FRAME_COUNT} frames"
        );
    }

    fn strip_reference_render_count_for_test(
        ctx: &PreviewRenderContext,
        fixture: PixelFixture,
    ) -> usize {
        let mut provider = PixelArtReferenceProvider::default();
        for index in 0..STRIP_FRAME_COUNT {
            let elapsed_ms = elapsed_for_index(index);
            let _ = render_pixel_artifact_with_provider(
                ctx,
                fixture,
                elapsed_ms,
                ctx.fixed_now,
                &mut provider,
            );
        }
        provider.render_count_for_test()
    }
}
