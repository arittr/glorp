use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles,
    PreviewStripKind,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::dev_preview::scenarios::PreviewRenderContext;
use crate::pet::animator::{SceneAnimator, SceneEffectTargets};
use crate::tui::component::layout_watch_with_context;
use crate::tui::layout::{render_watch_frame_with_layout, scene_effect_targets_from_layout};
use crate::tui::render_context::{RenderContext, WatchClock};
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;
use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    layout::{Position, Rect},
    style::{Color, Style},
    Terminal,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tachyonfx::{Duration as FxDuration, Effect, EffectTimer, Interpolation};

#[derive(Debug, Clone)]
pub struct PreviewStripBundle {
    pub manifest: PreviewStrip,
    pub frames: Vec<PreviewFrame>,
}

pub fn strip_frame_paths(strip_id: &str, index: usize) -> PreviewStripFrameFiles {
    PreviewStripFrameFiles {
        text: PathBuf::from(format!("strips/{strip_id}/frame-{index:03}.txt")),
        cells: PathBuf::from(format!("strips/{strip_id}/frame-{index:03}.cells.json")),
        locomotion: None,
        pixel: None,
    }
}

pub fn scene_strip_smoke() -> PreviewStripBundle {
    let phases = [
        ("start", 0_u16, "."),
        ("mid", 350_u16, "*"),
        ("end", 700_u16, "·"),
    ];
    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();

    for (index, (phase, elapsed_ms, glyph)) in phases.into_iter().enumerate() {
        let frame_id = format!("scene-strip-smoke-frame-{index:03}");
        let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));
        for x in 4..36 {
            buffer[(x, 4)]
                .set_symbol(glyph)
                .set_style(Style::default().fg(Color::Yellow));
        }
        let frame = frame_from_buffer(frame_id, format!("Scene Strip Smoke {phase}"), &buffer);
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: index as u16,
            phase: phase.to_string(),
            elapsed_ms,
            files: strip_frame_paths("scene-strip-smoke", index),
        });
    }

    PreviewStripBundle {
        manifest: PreviewStrip {
            id: "scene-strip-smoke".to_string(),
            kind: PreviewStripKind::SceneMoment,
            title: "Scene Strip Smoke".to_string(),
            intent: "Proves Preview Lab can export and play deterministic scene strips."
                .to_string(),
            dimensions: PreviewDimensions { width: 40, height: 8 },
            target_id: "watch.room.effect".to_string(),
            playback: PreviewPlayback {
                starts_paused: true,
                frame_duration_ms: 160,
            },
            inputs: BTreeMap::from([
                (
                    "fixture".to_string(),
                    Value::String("strip-smoke".to_string()),
                ),
                ("elapsed_ms".to_string(), json!([0, 350, 700])),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Confirm playback starts paused.".to_string(),
                "Step through start, mid, and end frames.".to_string(),
            ],
        },
        frames,
    }
}

/// Build a deterministic scene strip by rendering a watch frame, applying a
/// `SceneAnimator` for the supplied moment, and sampling at start / mid / end.
#[allow(clippy::too_many_arguments)]
fn scene_strip_bundle(
    strip_id: &'static str,
    title: &'static str,
    target_id: &'static str,
    intent: &'static str,
    vm: &WatchViewModel,
    width: u16,
    height: u16,
    moment: &crate::tui::room::SceneMoment,
    fixed_now: time::OffsetDateTime,
) -> PreviewStripBundle {
    let ctx = RenderContext::with_clock(ColorCapability::Truecolor, WatchClock::fixed(fixed_now));
    let layout = layout_watch_with_context(Rect::new(0, 0, width, height), vm, &ctx);
    let targets = scene_effect_targets_from_layout(&layout);

    let mut animator = SceneAnimator::new();
    let mut deterministic_effect =
        deterministic_preview_effect_for_moment(moment, &targets, fixed_now, strip_id);
    if deterministic_effect.is_none() {
        animator.update_scene_moments(std::slice::from_ref(moment), &targets);
    }

    let duration = moment.duration_ms as u32;
    let samples = [0_u32, duration / 2, duration];
    let phases = ["start", "mid", "end"];
    let mut prev_elapsed = 0_u32;

    let mut frames = Vec::new();
    let mut manifest_frames = Vec::new();

    for (index, &target_elapsed) in samples.iter().enumerate() {
        let delta = target_elapsed - prev_elapsed;
        prev_elapsed = target_elapsed;

        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                render_watch_frame_with_layout(frame, vm, &ctx, &layout);
                if let Some(effect) = deterministic_effect.as_mut() {
                    effect.process(
                        FxDuration::from_millis(delta),
                        frame.buffer_mut(),
                        targets.frame,
                    );
                } else {
                    animator.apply(&targets, frame.buffer_mut(), delta);
                }
            })
            .unwrap();

        let frame_id = format!("{strip_id}-frame-{index:03}");
        let frame = frame_from_buffer(
            frame_id,
            format!("{title} {}", phases[index]),
            terminal.backend().buffer(),
        );
        frames.push(frame);
        manifest_frames.push(PreviewStripFrame {
            index: index as u16,
            phase: phases[index].to_string(),
            elapsed_ms: target_elapsed as u16,
            files: strip_frame_paths(strip_id, index),
        });
    }

    PreviewStripBundle {
        manifest: PreviewStrip {
            id: strip_id.to_string(),
            kind: PreviewStripKind::SceneMoment,
            title: title.to_string(),
            intent: intent.to_string(),
            dimensions: PreviewDimensions { width, height },
            target_id: target_id.to_string(),
            playback: PreviewPlayback {
                starts_paused: true,
                frame_duration_ms: 160,
            },
            inputs: BTreeMap::from([
                ("fixture".to_string(), Value::String(strip_id.to_string())),
                ("elapsed_ms".to_string(), json!(samples)),
            ]),
            frames: manifest_frames,
            review_prompts: vec![
                "Confirm playback starts paused.".to_string(),
                "Step through start, mid, and end frames.".to_string(),
            ],
        },
        frames,
    }
}

fn deterministic_preview_effect_for_moment(
    moment: &crate::tui::room::SceneMoment,
    targets: &SceneEffectTargets,
    fixed_now: time::OffsetDateTime,
    strip_id: &str,
) -> Option<Effect> {
    if moment.key != crate::tui::room::SceneMomentKey::PropResonanceRipple {
        return None;
    }

    targets.props.get(moment.target_id).map(|rect| {
        deterministic_coalesce(fixed_now, strip_id, moment.duration_ms as u32).with_area(*rect)
    })
}

fn deterministic_coalesce(
    fixed_now: time::OffsetDateTime,
    strip_id: &str,
    duration_ms: u32,
) -> Effect {
    let seed = deterministic_effect_seed(fixed_now, strip_id);
    let timer = EffectTimer::from_ms(duration_ms, Interpolation::Linear).mirrored();
    tachyonfx::fx::effect_fn(seed, timer, |seed, ctx, cells| {
        let alpha = ctx.alpha();
        cells.for_each_cell(|pos, cell| {
            if alpha > deterministic_cell_fraction(*seed, pos) {
                cell.set_char(' ');
            }
        });
    })
}

fn deterministic_effect_seed(fixed_now: time::OffsetDateTime, strip_id: &str) -> u32 {
    let mut seed = fixed_now.unix_timestamp() as u32;
    for byte in strip_id.bytes() {
        seed = seed.wrapping_mul(16_777_619) ^ u32::from(byte);
    }
    seed
}

fn deterministic_cell_fraction(seed: u32, pos: Position) -> f32 {
    let mut value = seed
        ^ u32::from(pos.x).wrapping_mul(0x9E37_79B9)
        ^ u32::from(pos.y).wrapping_mul(0x85EB_CA6B);
    value ^= value >> 16;
    value = value.wrapping_mul(0x7FEB_352D);
    value ^= value >> 15;
    value = value.wrapping_mul(0x846C_A68B);
    value ^= value >> 16;
    f32::from_bits(0x3F80_0000 | (value >> 9)) - 1.0
}

pub fn scene_prop_resonance_ripple(ctx: &PreviewRenderContext) -> PreviewStripBundle {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::PropResonanceRipple,
        trigger_id: crate::tui::room::SceneTriggerId::new("prop:codex_signal_lamp:1"),
        target_id: "watch.prop.codex_signal_lamp.effect",
        duration_ms: 700,
    };
    scene_strip_bundle(
        "scene-prop-resonance-ripple",
        "Prop Resonance Ripple",
        "watch.prop.codex_signal_lamp.effect",
        "Shows a prop-backed resonance ripple on the codex signal lamp.",
        &vm,
        120,
        32,
        &moment,
        ctx.fixed_now,
    )
}

pub fn scene_feed_sweep(ctx: &PreviewRenderContext) -> PreviewStripBundle {
    let mut vm = WatchViewModel::fixture();
    vm.life_profile.burst_level = 0.8;
    vm.last_feed_pulse_at = Some(time::OffsetDateTime::from_unix_timestamp(1_000).unwrap());
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::FeedSweep,
        trigger_id: crate::tui::room::SceneTriggerId::new("feed:1000"),
        target_id: "watch.pet.effect",
        duration_ms: 500,
    };
    scene_strip_bundle(
        "scene-feed-sweep",
        "Feed Sweep",
        "watch.pet.effect",
        "Shows a feed sweep across the pet area after a usage burst.",
        &vm,
        120,
        32,
        &moment,
        ctx.fixed_now,
    )
}

pub fn scene_dawn_wake_wipe(ctx: &PreviewRenderContext) -> PreviewStripBundle {
    let vm = WatchViewModel::fixture();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::DawnWakeWipe,
        trigger_id: crate::tui::room::SceneTriggerId::new("wake:1"),
        target_id: "watch.room.effect",
        duration_ms: 900,
    };
    scene_strip_bundle(
        "scene-dawn-wake-wipe",
        "Dawn Wake Wipe",
        "watch.room.effect",
        "Shows a dawn wake wipe across the room background.",
        &vm,
        120,
        32,
        &moment,
        ctx.fixed_now,
    )
}

pub fn scene_heavy_session_shimmer(ctx: &PreviewRenderContext) -> PreviewStripBundle {
    let vm = WatchViewModel::fixture();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::HeavySessionShimmer,
        trigger_id: crate::tui::room::SceneTriggerId::new("heavy:1"),
        target_id: "watch.room.effect",
        duration_ms: 600,
    };
    scene_strip_bundle(
        "scene-heavy-session-shimmer",
        "Heavy Session Shimmer",
        "watch.room.effect",
        "Shows a shimmer across the room during a heavy session.",
        &vm,
        120,
        32,
        &moment,
        ctx.fixed_now,
    )
}

/// Return all real scene strip bundles.
pub fn scene_strips(ctx: &PreviewRenderContext) -> Vec<PreviewStripBundle> {
    vec![
        scene_prop_resonance_ripple(ctx),
        scene_feed_sweep(ctx),
        scene_dawn_wake_wipe(ctx),
        scene_heavy_session_shimmer(ctx),
    ]
}
