use crate::dev_preview::export::{
    PreviewDimensions, PreviewPixelFrameArtifact, PreviewPlayback, PreviewScenarioKind,
    PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles, PreviewStripKind,
    PIXEL_FRAME_SCHEMA_VERSION,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::{PreviewRenderContext, PreviewScenarioBundle};
use crate::dev_preview::strips::PreviewStripBundle;
use crate::game::{evolution::Stage, metabolism::Mood};
use crate::pet::{art::stage_label, generation::Species};
use crate::presentation::pixel::{
    render_pixel_frame, PixelFrame, PixelPetInput, PixelPetScene, PixelRendererState,
    PixelRendererTick, PixelViewport,
};
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
            &["species fuzz", "stage s3 archfuzz", "mood content", "pose idle"],
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
    let (artifact, input) = render_pixel_artifact(ctx, fixture, fixture.elapsed_ms);
    let mut frame = summary_frame(fixture.id, fixture.title, lines);
    frame.contract.pixel = Some(artifact);

    PreviewScenarioBundle::from_parts(
        frame,
        PreviewScenarioKind::Pixel,
        intent,
        scenario_inputs(&input, fixture.elapsed_ms),
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

    for index in 0..STRIP_FRAME_COUNT {
        let elapsed_ms = elapsed_for_index(index);
        let (artifact, input) = render_pixel_artifact(ctx, fixture, elapsed_ms);
        let scene = pixel_scene_for_elapsed(&input, elapsed_ms);
        let mut frame = strip_placeholder_frame(
            &format!("{}-frame-{index:03}", fixture.id),
            &phase_for_scene(&input, scene),
        );
        frame.contract.pixel = Some(artifact);
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
                &fixture_input(fixture, ctx.fixed_now + time::Duration::milliseconds(0)),
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
) -> (PreviewPixelFrameArtifact, PixelPetInput) {
    let base = ctx.fixed_now;
    let now = base + time::Duration::milliseconds(i64::from(elapsed_ms));
    let input = fixture_input(fixture, now);
    let mut state = PixelRendererState::new(&input, base);
    let frame = render_pixel_frame(PixelRendererTick {
        input: &input,
        viewport: PixelViewport::companion_default(),
        now,
        state: &mut state,
    });
    (pixel_artifact(&frame, &input, elapsed_ms), input)
}

fn fixture_input(fixture: PixelFixture, now: time::OffsetDateTime) -> PixelPetInput {
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
        .map(|age_ms| now - time::Duration::milliseconds(i64::from(age_ms)));
    PixelPetInput::from_watch_view_model(&vm, now)
}

fn scenario_inputs(input: &PixelPetInput, elapsed_ms: u16) -> BTreeMap<String, Value> {
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
    ])
}

fn summary_frame(id: &str, title: &str, lines: &[&str]) -> PreviewFrame {
    let width = 24;
    let height = lines.len().max(1) as u16;
    let mut buffer = Buffer::empty(Rect::new(0, 0, width, height));
    for (row, line) in lines.iter().enumerate() {
        buffer.set_string(0, row as u16, *line, Style::default());
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
