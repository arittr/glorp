use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles,
    PreviewStripKind,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use crate::pet::animator::SceneAnimator;
use crate::tui::component::layout_watch_with_context;
use crate::tui::layout::{render_watch_frame_with_layout, scene_effect_targets_from_layout};
use crate::tui::render_context::RenderContext;
use crate::tui::style::ColorCapability;
use crate::tui::view_model::WatchViewModel;
use ratatui::{
    backend::TestBackend,
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    Terminal,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PreviewStripBundle {
    pub manifest: PreviewStrip,
    pub frames: Vec<PreviewFrame>,
}

pub fn strip_frame_paths(strip_id: &str, index: usize) -> PreviewStripFrameFiles {
    PreviewStripFrameFiles {
        text: PathBuf::from(format!("strips/{strip_id}/frame-{index:03}.txt")),
        cells: PathBuf::from(format!("strips/{strip_id}/frame-{index:03}.cells.json")),
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
            dimensions: PreviewDimensions {
                width: 40,
                height: 8,
            },
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
) -> PreviewStripBundle {
    let ctx = RenderContext::new(ColorCapability::Truecolor);
    let layout = layout_watch_with_context(Rect::new(0, 0, width, height), vm, &ctx);
    let targets = scene_effect_targets_from_layout(&layout);

    let mut animator = SceneAnimator::new();
    animator.update_scene_moments(std::slice::from_ref(moment), &targets);

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
                animator.apply(&targets, frame.buffer_mut(), delta);
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

pub fn scene_prop_resonance_ripple() -> PreviewStripBundle {
    let vm = WatchViewModel::fixture_with_habitat_props();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::PropResonanceRipple,
        trigger_id: crate::tui::room::SceneTriggerId::new("prop:codex_signal_lamp:1"),
        target_id: "watch.prop.codex_signal_lamp.effect",
        duration_ms: 700,
        max_replay_age_ms: 3_600_000,
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
    )
}

pub fn scene_feed_sweep() -> PreviewStripBundle {
    let mut vm = WatchViewModel::fixture();
    vm.life_profile.burst_level = 0.8;
    vm.last_feed_pulse_at = Some(time::OffsetDateTime::from_unix_timestamp(1_000).unwrap());
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::FeedSweep,
        trigger_id: crate::tui::room::SceneTriggerId::new("feed:1000"),
        target_id: "watch.pet.effect",
        duration_ms: 500,
        max_replay_age_ms: 8_000,
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
    )
}

pub fn scene_dawn_wake_wipe() -> PreviewStripBundle {
    let vm = WatchViewModel::fixture();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::DawnWakeWipe,
        trigger_id: crate::tui::room::SceneTriggerId::new("wake:1"),
        target_id: "watch.room.effect",
        duration_ms: 900,
        max_replay_age_ms: 3_600_000,
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
    )
}

pub fn scene_heavy_session_shimmer() -> PreviewStripBundle {
    let vm = WatchViewModel::fixture();
    let moment = crate::tui::room::SceneMoment {
        key: crate::tui::room::SceneMomentKey::HeavySessionShimmer,
        trigger_id: crate::tui::room::SceneTriggerId::new("heavy:1"),
        target_id: "watch.room.effect",
        duration_ms: 600,
        max_replay_age_ms: 3_600_000,
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
    )
}

/// Return all real scene strip bundles.
pub fn scene_strips() -> Vec<PreviewStripBundle> {
    vec![
        scene_prop_resonance_ripple(),
        scene_feed_sweep(),
        scene_dawn_wake_wipe(),
        scene_heavy_session_shimmer(),
    ]
}
