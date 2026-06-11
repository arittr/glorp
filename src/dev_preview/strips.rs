use crate::dev_preview::export::{
    PreviewDimensions, PreviewPlayback, PreviewStrip, PreviewStripFrame, PreviewStripFrameFiles,
    PreviewStripKind,
};
use crate::dev_preview::frame::{frame_from_buffer, PreviewFrame};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
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
