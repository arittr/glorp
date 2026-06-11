use crate::tui::component::{ComponentLayout, GeometryTarget, LayoutMode, TargetPath, TargetRole};
use ratatui::layout::Rect;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewLayout {
    pub schema_version: u32,
    pub frame_id: String,
    pub mode: String,
    pub frame: PreviewRect,
    pub content: PreviewRect,
    pub components: BTreeMap<String, PreviewRect>,
    pub targets: BTreeMap<String, PreviewTarget>,
    pub decisions: Vec<PreviewLayoutDecision>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct PreviewRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewTarget {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub owner: String,
    pub role: String,
    pub clip: PreviewRect,
    pub z: i16,
    pub layer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cell_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PreviewLayoutDecision {
    pub path: String,
    pub reason: String,
    pub message: String,
}

pub fn preview_layout(frame_id: &str, layout: &ComponentLayout) -> PreviewLayout {
    PreviewLayout {
        schema_version: 2,
        frame_id: frame_id.to_string(),
        mode: mode_name(layout.mode).to_string(),
        frame: rect(layout.frame),
        content: rect(layout.content),
        components: layout
            .nodes
            .iter()
            .map(|(path, node)| (path.as_str().to_string(), rect(node.bounds)))
            .collect(),
        targets: layout
            .targets
            .iter()
            .map(|(path, target)| (path.as_str().to_string(), preview_target(path, target)))
            .collect(),
        decisions: layout
            .decisions
            .iter()
            .map(|decision| PreviewLayoutDecision {
                path: decision.path.as_str().to_string(),
                reason: format!("{:?}", decision.reason),
                message: decision.message.to_string(),
            })
            .collect(),
    }
}

fn mode_name(mode: LayoutMode) -> &'static str {
    match mode {
        LayoutMode::Wide => "wide",
        LayoutMode::Compact => "compact",
    }
}

fn rect(rect: Rect) -> PreviewRect {
    PreviewRect {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
    }
}

fn preview_target(_path: &TargetPath, target: &GeometryTarget) -> PreviewTarget {
    PreviewTarget {
        x: target.rect.x,
        y: target.rect.y,
        width: target.rect.width,
        height: target.rect.height,
        owner: target.owner.as_str().to_string(),
        role: format!("{:?}", target.role),
        clip: rect(target.clip),
        z: target.z,
        layer: match target.role {
            TargetRole::RoomEffect => "room-background".to_string(),
            TargetRole::PropEffect => "prop".to_string(),
            _ => "component".to_string(),
        },
        cell_count: target.cell_count,
    }
}
