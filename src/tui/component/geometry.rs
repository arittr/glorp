use crate::tui::component::ids::{ComponentPath, TargetPath};
use ratatui::layout::{Position, Rect};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Wide,
    Compact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentLayout {
    pub frame: Rect,
    pub content: Rect,
    pub mode: LayoutMode,
    pub nodes: BTreeMap<ComponentPath, ComponentNodeLayout>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
    pub decisions: Vec<LayoutDecision>,
}

impl ComponentLayout {
    pub fn new(frame: Rect, mode: LayoutMode) -> Self {
        Self {
            frame,
            content: frame,
            mode,
            nodes: BTreeMap::new(),
            targets: BTreeMap::new(),
            decisions: Vec::new(),
        }
    }

    pub fn with_content(mut self, content: Rect) -> Self {
        self.content = content;
        self
    }

    pub fn insert_node(&mut self, node: ComponentNodeLayout) -> Result<(), LayoutBuildError> {
        if self.nodes.contains_key(&node.id) {
            return Err(LayoutBuildError::DuplicateComponent(node.id));
        }
        self.nodes.insert(node.id, node);
        Ok(())
    }

    pub fn insert_target(
        &mut self,
        path: TargetPath,
        target: GeometryTarget,
    ) -> Result<(), LayoutBuildError> {
        if self.targets.contains_key(&path) {
            return Err(LayoutBuildError::DuplicateTarget(path));
        }
        let Some(owner_node) = self.nodes.get(&target.owner) else {
            return Err(LayoutBuildError::UnknownTargetOwner(target.owner));
        };
        if owner_node.targets.contains_key(&path) {
            return Err(LayoutBuildError::DuplicateTarget(path));
        }
        self.targets.insert(path, target);
        let owner_node = self
            .nodes
            .get_mut(&target.owner)
            .expect("target owner was validated before insertion");
        owner_node.targets.insert(path, target);
        Ok(())
    }

    pub fn node(&self, id: ComponentPath) -> Option<&ComponentNodeLayout> {
        self.nodes.get(&id)
    }

    pub fn target(&self, path: TargetPath) -> Option<&GeometryTarget> {
        self.targets.get(&path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentNodeLayout {
    pub id: ComponentPath,
    pub bounds: Rect,
    pub content: Rect,
    pub visibility: VisibilityState,
    pub children: Vec<ComponentPath>,
    pub targets: BTreeMap<TargetPath, GeometryTarget>,
}

impl ComponentNodeLayout {
    pub fn leaf(id: ComponentPath, bounds: Rect) -> Self {
        Self {
            id,
            bounds,
            content: bounds,
            visibility: VisibilityState::Visible,
            children: Vec::new(),
            targets: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryTarget {
    pub owner: ComponentPath,
    pub rect: Rect,
    pub z: i16,
    pub clip: Rect,
    pub role: TargetRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetRole {
    Content,
    PetPanel,
    PetArt,
    PetSpeech,
    Habitat,
    Effect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityState {
    Visible,
    Hidden { reason: LayoutDecisionReason },
    Degraded { reason: LayoutDecisionReason },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDecisionReason {
    CompactMode,
    InsufficientHeight,
    InsufficientWidth,
    RowLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutDecision {
    pub path: ComponentPath,
    pub reason: LayoutDecisionReason,
    pub message: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitResult {
    pub target: TargetPath,
    pub rect: Rect,
    pub local_position: Position,
    pub z: i16,
}

pub fn hit_test(layout: &ComponentLayout, point: Position) -> Option<HitResult> {
    layout
        .targets
        .iter()
        .filter(|(_, target)| {
            rect_contains(target.rect, point) && rect_contains(target.clip, point)
        })
        .max_by_key(|(_, target)| target.z)
        .map(|(path, target)| HitResult {
            target: *path,
            rect: target.rect,
            local_position: Position::new(point.x - target.rect.x, point.y - target.rect.y),
            z: target.z,
        })
}

fn rect_contains(rect: Rect, point: Position) -> bool {
    point.x >= rect.x
        && point.y >= rect.y
        && point.x < rect.x.saturating_add(rect.width)
        && point.y < rect.y.saturating_add(rect.height)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutBuildError {
    DuplicateComponent(ComponentPath),
    DuplicateTarget(TargetPath),
    UnknownTargetOwner(ComponentPath),
}

impl fmt::Display for LayoutBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutBuildError::DuplicateComponent(path) => {
                write!(f, "duplicate component path {path}")
            }
            LayoutBuildError::DuplicateTarget(path) => write!(f, "duplicate target path {path}"),
            LayoutBuildError::UnknownTargetOwner(path) => {
                write!(f, "unknown target owner {path}")
            }
        }
    }
}

impl std::error::Error for LayoutBuildError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::component::ids::{ComponentPath, TargetPath};
    use ratatui::layout::{Position, Rect};

    #[test]
    fn component_layout_rejects_duplicate_nodes() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        let node =
            ComponentNodeLayout::leaf(ComponentPath::new("watch.pet"), Rect::new(0, 0, 10, 10));
        layout.insert_node(node.clone()).unwrap();
        let err = layout.insert_node(node).unwrap_err();
        assert!(err.to_string().contains("duplicate component path"));
    }

    #[test]
    fn component_layout_indexes_targets_by_stable_path() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout
            .insert_node(ComponentNodeLayout::leaf(
                ComponentPath::new("watch.pet"),
                Rect::new(1, 2, 20, 10),
            ))
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.art"),
                GeometryTarget {
                    owner: ComponentPath::new("watch.pet"),
                    rect: Rect::new(5, 4, 13, 10),
                    z: 10,
                    clip: Rect::new(1, 2, 20, 10),
                    role: TargetRole::PetArt,
                },
            )
            .unwrap();
        assert_eq!(
            layout
                .target(TargetPath::new("watch.pet.art"))
                .unwrap()
                .rect
                .width,
            13
        );
    }

    #[test]
    fn component_layout_populates_owner_node_targets() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout
            .insert_node(ComponentNodeLayout::leaf(
                ComponentPath::new("watch.pet"),
                Rect::new(1, 2, 20, 10),
            ))
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.art"),
                GeometryTarget {
                    owner: ComponentPath::new("watch.pet"),
                    rect: Rect::new(5, 4, 13, 10),
                    z: 10,
                    clip: Rect::new(1, 2, 20, 10),
                    role: TargetRole::PetArt,
                },
            )
            .unwrap();

        let node = layout.node(ComponentPath::new("watch.pet")).unwrap();
        assert_eq!(
            node.targets
                .get(&TargetPath::new("watch.pet.art"))
                .unwrap()
                .rect,
            Rect::new(5, 4, 13, 10)
        );
    }

    #[test]
    fn component_layout_rejects_duplicate_targets_without_mutating_owner_node() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout
            .insert_node(ComponentNodeLayout::leaf(
                ComponentPath::new("watch.pet"),
                Rect::new(1, 2, 20, 10),
            ))
            .unwrap();
        let target = GeometryTarget {
            owner: ComponentPath::new("watch.pet"),
            rect: Rect::new(5, 4, 13, 10),
            z: 10,
            clip: Rect::new(1, 2, 20, 10),
            role: TargetRole::PetArt,
        };
        layout
            .insert_target(TargetPath::new("watch.pet.art"), target)
            .unwrap();

        let err = layout
            .insert_target(
                TargetPath::new("watch.pet.art"),
                GeometryTarget { z: 20, ..target },
            )
            .unwrap_err();

        assert!(err.to_string().contains("duplicate target path"));
        let node = layout.node(ComponentPath::new("watch.pet")).unwrap();
        assert_eq!(node.targets.len(), 1);
        assert_eq!(
            node.targets
                .get(&TargetPath::new("watch.pet.art"))
                .unwrap()
                .z,
            10
        );
    }

    #[test]
    fn component_layout_rejects_targets_with_unknown_owner() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        let err = layout
            .insert_target(
                TargetPath::new("watch.pet.art"),
                GeometryTarget {
                    owner: ComponentPath::new("watch.pet"),
                    rect: Rect::new(5, 4, 13, 10),
                    z: 10,
                    clip: Rect::new(1, 2, 20, 10),
                    role: TargetRole::PetArt,
                },
            )
            .unwrap_err();

        assert!(err.to_string().contains("unknown target owner"));
        assert!(layout.target(TargetPath::new("watch.pet.art")).is_none());
    }

    #[test]
    fn component_layout_hit_test_returns_highest_z_target_containing_point() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout
            .insert_node(ComponentNodeLayout::leaf(
                ComponentPath::new("watch.pet"),
                Rect::new(0, 0, 20, 20),
            ))
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.panel"),
                GeometryTarget {
                    owner: ComponentPath::new("watch.pet"),
                    rect: Rect::new(0, 0, 20, 20),
                    z: 1,
                    clip: Rect::new(0, 0, 20, 20),
                    role: TargetRole::PetPanel,
                },
            )
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.art"),
                GeometryTarget {
                    owner: ComponentPath::new("watch.pet"),
                    rect: Rect::new(4, 5, 13, 10),
                    z: 10,
                    clip: Rect::new(0, 0, 20, 20),
                    role: TargetRole::PetArt,
                },
            )
            .unwrap();

        let hit = hit_test(&layout, Position::new(6, 6)).unwrap();
        assert_eq!(hit.target, TargetPath::new("watch.pet.art"));
        assert_eq!(hit.local_position, Position::new(2, 1));
    }

    #[test]
    fn component_layout_hit_test_ignores_unclipped_target_area() {
        let mut layout = ComponentLayout::new(Rect::new(0, 0, 80, 24), LayoutMode::Compact);
        layout
            .insert_node(ComponentNodeLayout::leaf(
                ComponentPath::new("watch.pet"),
                Rect::new(0, 0, 20, 20),
            ))
            .unwrap();
        layout
            .insert_target(
                TargetPath::new("watch.pet.art"),
                GeometryTarget {
                    owner: ComponentPath::new("watch.pet"),
                    rect: Rect::new(0, 0, 20, 20),
                    z: 10,
                    clip: Rect::new(0, 0, 10, 10),
                    role: TargetRole::PetArt,
                },
            )
            .unwrap();

        assert!(hit_test(&layout, Position::new(12, 5)).is_none());
        assert_eq!(
            hit_test(&layout, Position::new(5, 5)).unwrap().target,
            TargetPath::new("watch.pet.art")
        );
    }
}
